// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "test-device.h"

#include <dirent.h>
#include <errno.h>
#include <fidl/fuchsia.hardware.ramdisk/cpp/wire.h>
#include <inttypes.h>
#include <lib/fdio/cpp/caller.h>
#include <lib/fdio/unsafe.h>
#include <lib/fdio/watcher.h>
#include <lib/fit/defer.h>
#include <lib/zircon-internal/debug.h>
#include <lib/zx/channel.h>
#include <lib/zx/clock.h>
#include <lib/zx/fifo.h>
#include <lib/zx/vmo.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <threads.h>
#include <unistd.h>
#include <zircon/assert.h>
#include <zircon/types.h>

#include <memory>
#include <string_view>

#include <fbl/algorithm.h>
#include <fbl/auto_lock.h>
#include <fbl/string.h>
#include <fbl/unique_fd.h>
#include <ramdevice-client/ramdisk.h>
#include <zxtest/zxtest.h>

#include "lib/stdcompat/string_view.h"
#include "src/security/lib/zxcrypt/client.h"
#include "src/security/lib/zxcrypt/fdio-volume.h"
#include "src/security/lib/zxcrypt/volume.h"
#include "src/storage/fvm/format.h"
#include "src/storage/lib/block_client/cpp/remote_block_device.h"
#include "src/storage/lib/fs_management/cpp/fvm.h"

#define ZXDEBUG 0

namespace zxcrypt {
namespace testing {
namespace {

// No test step should take longer than this
const zx::duration kTimeout = zx::sec(3);

// FVM driver library
const char* kFvmDriver = "fvm.cm";

}  // namespace

TestDevice::TestDevice() {
  memset(fvm_part_path_, 0, sizeof(fvm_part_path_));
  memset(&req_, 0, sizeof(req_));
}

TestDevice::~TestDevice() {
  Disconnect();
  DestroyRamdisk();
  if (need_join_) {
    int res;
    thrd_join(tid_, &res);
  }
}

void TestDevice::SetupDevmgr() {
  driver_integration_test::IsolatedDevmgr::Args args;

  // We explicitly bind drivers ourselves, and don't want the block watcher
  // racing with us to call Bind.
  args.disable_block_watcher = true;

  ASSERT_EQ(driver_integration_test::IsolatedDevmgr::Create(&args, &devmgr_), ZX_OK);
  ASSERT_EQ(device_watcher::RecursiveWaitForFile(devmgr_.devfs_root().get(),
                                                 "sys/platform/ram-disk/ramctl")
                .status_value(),
            ZX_OK);
}

void TestDevice::Create(size_t device_size, size_t block_size, bool fvm, Volume::Version version) {
  ASSERT_LT(device_size, SSIZE_MAX);
  if (fvm) {
    ASSERT_NO_FATAL_FAILURE(CreateFvmPart(device_size, block_size));
  } else {
    ASSERT_NO_FATAL_FAILURE(CreateRamdisk(device_size, block_size));
  }

  crypto::digest::Algorithm digest;
  switch (version) {
    case Volume::kAES256_XTS_SHA256:
      digest = crypto::digest::kSHA256;
      break;
    default:
      digest = crypto::digest::kUninitialized;
      break;
  }

  size_t digest_len;
  key_.Clear();
  ASSERT_OK(crypto::digest::GetDigestLen(digest, &digest_len));
  ASSERT_OK(key_.Generate(digest_len));
}

void TestDevice::Bind(Volume::Version version, bool fvm) {
  ASSERT_NO_FATAL_FAILURE(Create(kDeviceSize, kBlockSize, fvm, version));

  zxcrypt::VolumeManager volume_manager(new_parent_controller(), devfs_root().duplicate());
  zx::channel zxc_client_chan;
  ASSERT_OK(volume_manager.OpenClient(kTimeout, zxc_client_chan));
  EncryptedVolumeClient volume_client(std::move(zxc_client_chan));
  ASSERT_OK(volume_client.Format(key_.get(), key_.len(), 0));

  ASSERT_NO_FATAL_FAILURE(Connect());
}

void TestDevice::BindFvmDriver() {
  // Binds the FVM driver to the active ramdisk_.
  const fidl::UnownedClientEnd<fuchsia_device::Controller> channel(
      ramdisk_get_block_controller_interface(ramdisk_));
  const fidl::WireResult result =
      fidl::WireCall(channel)->Bind(fidl::StringView::FromExternal(kFvmDriver));
  ASSERT_OK(result.status());
  const fit::result response = result.value();
  ASSERT_TRUE(response.is_ok(), "%s", zx_status_get_string(response.error_value()));
}

void TestDevice::Rebind() {
  const char* sep = strrchr(ramdisk_get_path(ramdisk_), '/');
  ASSERT_NOT_NULL(sep);

  Disconnect();
  zxcrypt_controller_.reset();
  zxcrypt_volume_.reset();
  fvm_.reset();

  if (strlen(fvm_part_path_) != 0) {
    const fidl::UnownedClientEnd<fuchsia_device::Controller> channel(
        ramdisk_get_block_controller_interface(ramdisk_));
    // We need to explicitly rebind FVM here, since now that we're not
    // relying on the system-wide block-watcher, the driver won't rebind by
    // itself.
    const fidl::WireResult result =
        fidl::WireCall(channel)->Rebind(fidl::StringView::FromExternal(kFvmDriver));
    ASSERT_OK(result.status());
    const fit::result response = result.value();
    ASSERT_TRUE(response.is_ok(), "%s", zx_status_get_string(response.error_value()));

    zx::result owned = device_watcher::RecursiveWaitForFile(devfs_root().get(), fvm_part_path_);
    ASSERT_OK(owned);
    fvm_ = fidl::ClientEnd<fuchsia_hardware_block_volume::Volume>(std::move(owned.value()));

    std::string controller_path = std::string(fvm_part_path_) + "/device_controller";
    zx::result controller =
        device_watcher::RecursiveWaitForFile(devfs_root().get(), controller_path.c_str());
    ASSERT_OK(controller);
    fvm_controller_ = fidl::ClientEnd<fuchsia_device::Controller>(std::move(controller.value()));
  } else {
    ASSERT_OK(ramdisk_rebind(ramdisk_));
    const fidl::UnownedClientEnd<fuchsia_device::Controller> channel(
        ramdisk_get_block_controller_interface(ramdisk_));
    zx::result server = fidl::CreateEndpoints(&fvm_controller_);
    ASSERT_OK(server);
    ASSERT_OK(fidl::WireCall(channel)->ConnectToController(std::move(server.value())).status());

    zx::result fvm = fidl::CreateEndpoints(&fvm_);
    ASSERT_OK(fvm);
    ASSERT_OK(
        fidl::WireCall(fvm_controller_)->ConnectToDeviceFidl(fvm.value().TakeChannel()).status());
  }
  ASSERT_NO_FATAL_FAILURE(Connect());
}

void TestDevice::SleepUntil(uint64_t num, bool deferred) {
  fbl::AutoLock lock(&lock_);
  ASSERT_EQ(wake_after_, 0);
  ASSERT_NE(num, 0);
  wake_after_ = num;
  wake_deadline_ = zx::deadline_after(kTimeout);
  ASSERT_EQ(thrd_create(&tid_, TestDevice::WakeThread, this), thrd_success);
  need_join_ = true;
  if (deferred) {
    uint32_t flags =
        static_cast<uint32_t>(fuchsia_hardware_ramdisk::wire::RamdiskFlag::kResumeOnWake);
    ASSERT_OK(ramdisk_set_flags(ramdisk_, flags));
  }
  uint64_t sleep_after = 0;
  ASSERT_OK(ramdisk_sleep_after(ramdisk_, sleep_after));
}

void TestDevice::WakeUp() {
  if (need_join_) {
    fbl::AutoLock lock(&lock_);
    ASSERT_NE(wake_after_, 0);
    int res;
    ASSERT_EQ(thrd_join(tid_, &res), thrd_success);
    need_join_ = false;
    wake_after_ = 0;
    EXPECT_EQ(res, 0);
  }
}

int TestDevice::WakeThread(void* arg) {
  TestDevice* device = static_cast<TestDevice*>(arg);
  fbl::AutoLock lock(&device->lock_);

  // Always send a wake-up call; even if we failed to go to sleep.
  auto cleanup = fit::defer([&] { ramdisk_wake(device->ramdisk_); });

  // Loop until timeout, |wake_after_| txns received, or error getting counts
  ramdisk_block_write_counts_t counts;
  do {
    zx::nanosleep(zx::deadline_after(zx::msec(100)));
    if (device->wake_deadline_ < zx::clock::get_monotonic()) {
      printf("Received %lu of %lu transactions before timing out.\n", counts.received,
             device->wake_after_);
      return ZX_ERR_TIMED_OUT;
    }
    zx_status_t status = ramdisk_get_block_counts(device->ramdisk_, &counts);
    if (status != ZX_OK) {
      return status;
    }
  } while (counts.received < device->wake_after_);
  return ZX_OK;
}

void TestDevice::Read(zx_off_t off, size_t len) {
  ASSERT_OK(SingleReadBytes(off, len, off));
  ASSERT_EQ(memcmp(as_read_.get() + off, to_write_.get() + off, len), 0);
}

void TestDevice::Write(zx_off_t off, size_t len) { ASSERT_OK(SingleWriteBytes(off, len, off)); }

void TestDevice::ReadVmo(zx_off_t off, size_t len) {
  ASSERT_OK(block_fifo_txn(BLOCK_OPCODE_READ, off, len));
  off *= block_size_;
  len *= block_size_;
  ASSERT_OK(vmo_read(off, len));
  ASSERT_EQ(memcmp(as_read_.get() + off, to_write_.get() + off, len), 0);
}

void TestDevice::WriteVmo(zx_off_t off, size_t len) {
  ASSERT_OK(vmo_write(off * block_size_, len * block_size_));
  ASSERT_OK(block_fifo_txn(BLOCK_OPCODE_WRITE, off, len));
}

void TestDevice::Corrupt(uint64_t blkno, key_slot_t slot) {
  uint8_t block[block_size_];

  // TODO(https://fxbug.dev/42080299): Update this API to take a volume channel instead.
  ASSERT_OK(block_client::SingleReadBytes(
      fidl::UnownedClientEnd<fuchsia_hardware_block::Block>(parent_volume().channel()), block,
      block_size_, blkno * block_size_));

  fidl::ClientEnd channel = new_parent();

  zx::result volume = FdioVolume::Unlock(std::move(channel), key_, 0);
  ASSERT_OK(volume);

  zx_off_t off;
  ASSERT_OK(volume->GetSlotOffset(slot, &off));
  int flip = 1U << (rand() % 8);
  block[off] ^= static_cast<uint8_t>(flip);

  // TODO(https://fxbug.dev/42080299): Update this API to take a volume channel instead.
  ASSERT_OK(block_client::SingleWriteBytes(
      fidl::UnownedClientEnd<fuchsia_hardware_block::Block>(parent_volume().channel()), block,
      block_size_, blkno * block_size_));
}

// Private methods

void TestDevice::CreateRamdisk(size_t device_size, size_t block_size) {
  fbl::AllocChecker ac;
  size_t count = fbl::round_up(device_size, block_size) / block_size;
  to_write_.reset(new (&ac) uint8_t[device_size]);
  ASSERT_TRUE(ac.check());
  for (size_t i = 0; i < device_size; ++i) {
    to_write_[i] = static_cast<uint8_t>(rand());
  }

  as_read_.reset(new (&ac) uint8_t[device_size]);
  ASSERT_TRUE(ac.check());
  memset(as_read_.get(), 0, block_size);

  ASSERT_EQ(ramdisk_create_at(devfs_root().get(), block_size, count, &ramdisk_), ZX_OK);

  zx::result owned =
      device_watcher::RecursiveWaitForFile(devfs_root().get(), ramdisk_get_path(ramdisk_));
  ASSERT_OK(owned);
  fvm_ = fidl::ClientEnd<fuchsia_hardware_block_volume::Volume>(std::move(owned.value()));

  std::string controller_path = std::string(ramdisk_get_path(ramdisk_)) + "/device_controller";
  zx::result controller =
      device_watcher::RecursiveWaitForFile(devfs_root().get(), controller_path.c_str());
  ASSERT_OK(controller);
  fvm_controller_ = fidl::ClientEnd<fuchsia_device::Controller>(std::move(controller.value()));

  block_size_ = block_size;
  block_count_ = count;
}

void TestDevice::DestroyRamdisk() {
  if (ramdisk_ != nullptr) {
    ramdisk_destroy(ramdisk_);
    ramdisk_ = nullptr;
  }
}

// Creates a ramdisk, formats it, and binds to it.
void TestDevice::CreateFvmPart(size_t device_size, size_t block_size) {
  // Calculate total size of data + metadata.
  size_t slice_count = fbl::round_up(device_size, fvm::kBlockSize) / fvm::kBlockSize;
  fvm::Header fvm_header =
      fvm::Header::FromSliceCount(fvm::kMaxUsablePartitions, slice_count, fvm::kBlockSize);

  ASSERT_NO_FATAL_FAILURE(CreateRamdisk(fvm_header.fvm_partition_size, block_size));

  // Format the ramdisk as FVM
  const fidl::UnownedClientEnd<fuchsia_hardware_block::Block> channel(
      ramdisk_get_block_interface(ramdisk_));
  ASSERT_OK(fs_management::FvmInit(channel, fvm::kBlockSize));

  // Bind the FVM driver to the now-formatted disk
  ASSERT_NO_FATAL_FAILURE(BindFvmDriver());

  // Wait for the FVM driver to expose a block device, then open it
  char path[PATH_MAX];
  snprintf(path, sizeof(path), "%s/fvm", ramdisk_get_path(ramdisk_));
  zx::result fvm_channel = device_watcher::RecursiveWaitForFile(devfs_root().get(), path);
  ASSERT_OK(fvm_channel.status_value());
  fidl::ClientEnd<fuchsia_hardware_block_volume::VolumeManager> fvm_manager(
      std::move(*fvm_channel));

  fdio_cpp::UnownedFdioCaller caller(devfs_root());
  zx::result devfs = component::Clone(caller.directory());
  ASSERT_OK(devfs.status_value());

  // Allocate a FVM partition with the last slice unallocated.
  uint64_t request_slice_count = (kDeviceSize / fvm::kBlockSize) - 1;
  zx::result fvm_part = fs_management::FvmAllocatePartitionWithDevfs(
      *devfs, fvm_manager, request_slice_count, uuid::Uuid(zxcrypt_magic), uuid::Uuid::Generate(),
      "data", 0);
  ASSERT_EQ(fvm_part.status_value(), ZX_OK);
  fvm_controller_ = std::move(fvm_part.value());

  {
    zx::result server = fidl::CreateEndpoints(&fvm_);
    ASSERT_OK(server);

    ASSERT_OK(fidl::WireCall(fvm_controller_)
                  ->ConnectToDeviceFidl(server.value().TakeChannel())
                  .status());
  }

  // Save the topological path for rebinding.  The topological path will be
  // consistent after rebinding the ramdisk, whereas the
  // /dev/class/block/[NNN] will issue a new number.
  const fidl::WireResult result = fidl::WireCall(parent_controller())->GetTopologicalPath();
  ASSERT_OK(result.status());
  const fit::result response = result.value();
  ASSERT_TRUE(response.is_ok(), "%s", zx_status_get_string(response.error_value()));
  std::string_view topological_path = response.value()->path.get();
  // Strip off the leading /dev/; because we use an isolated devmgr, we need
  // relative paths, but ControllerGetTopologicalPath returns an absolute path
  // with the assumption that devfs is rooted at /dev.
  constexpr std::string_view kHeader = "/dev/";
  ASSERT_TRUE(cpp20::starts_with(topological_path, kHeader));
  topological_path = topological_path.substr(kHeader.size());
  memcpy(fvm_part_path_, topological_path.data(), topological_path.size());
  fvm_part_path_[topological_path.size()] = 0;
}

void TestDevice::Connect() {
  ZX_DEBUG_ASSERT(!zxcrypt_volume_);

  volume_manager_ = zxcrypt::VolumeManager(new_parent_controller(), devfs_root().duplicate());
  zx::channel zxc_client_chan;
  ASSERT_OK(volume_manager_->OpenClient(kTimeout, zxc_client_chan));

  EncryptedVolumeClient volume_client(std::move(zxc_client_chan));
  zx_status_t rc;
  // Unseal may fail because the volume is already unsealed, so we also allow
  // ZX_ERR_INVALID_STATE here.  If we fail to unseal the volume, the
  // volume_->Open() call below will fail, so this is safe to ignore.
  rc = volume_client.Unseal(key_.get(), key_.len(), 0);
  ASSERT_TRUE(rc == ZX_OK || rc == ZX_ERR_BAD_STATE);
  zx::result zxcrypt = volume_manager_->OpenInnerBlockDevice(kTimeout);
  ASSERT_OK(zxcrypt);
  zxcrypt_controller_ = std::move(zxcrypt.value());
  zx::result zxcrypt_server = fidl::CreateEndpoints(&zxcrypt_volume_);
  ASSERT_OK(zxcrypt_server);
  ASSERT_OK(fidl::WireCall(zxcrypt_controller_)
                ->ConnectToDeviceFidl(zxcrypt_server.value().TakeChannel())
                .status());

  {
    const fidl::WireResult result = fidl::WireCall(zxcrypt_block())->GetInfo();
    ASSERT_OK(result.status());
    const fit::result response = result.value();
    ASSERT_TRUE(response.is_ok(), "%s", zx_status_get_string(response.error_value()));
    block_size_ = response.value()->info.block_size;
    block_count_ = response.value()->info.block_count;
  }

  auto [client, server] = fidl::Endpoints<fuchsia_hardware_block::Session>::Create();
  ASSERT_OK(fidl::WireCall(zxcrypt_block())->OpenSession(std::move(server)));

  {
    const fidl::WireResult result = fidl::WireCall(client)->GetFifo();
    ASSERT_OK(result.status());
    const fit::result response = result.value();
    ASSERT_TRUE(response.is_ok(), "%s", zx_status_get_string(response.error_value()));
    client_ = std::make_unique<block_client::Client>(std::move(client),
                                                     std::move(response.value()->fifo));
  }

  req_.group = 0;

  // Create the vmo and get a transferable handle to give to the block server
  ASSERT_OK(zx::vmo::create(size(), 0, &vmo_));
  zx::result vmoid = client_->RegisterVmo(vmo_);
  ASSERT_OK(vmoid);
  req_.vmoid = vmoid.value().TakeId();
}

void TestDevice::Disconnect() {
  if (volume_manager_) {
    zx::channel zxc_client_chan;
    volume_manager_->OpenClient(kTimeout, zxc_client_chan);
    if (zxc_client_chan) {
      EncryptedVolumeClient volume_client(std::move(zxc_client_chan));
      volume_client.Seal();
    }
  }

  if (client_) {
    memset(&req_, 0, sizeof(req_));
    client_ = nullptr;
  }
  zxcrypt_controller_.reset();
  zxcrypt_volume_.reset();

  volume_manager_.reset();
  block_size_ = 0;
  block_count_ = 0;
  vmo_.reset();
}

}  // namespace testing
}  // namespace zxcrypt
