// Copyright 2020 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#ifndef SRC_DEVICES_BUS_LIB_VIRTIO_INCLUDE_LIB_VIRTIO_DRIVER_UTILS_H_
#define SRC_DEVICES_BUS_LIB_VIRTIO_INCLUDE_LIB_VIRTIO_DRIVER_UTILS_H_

#include <lib/zx/result.h>
#include <stdlib.h>
#include <zircon/compiler.h>
#include <zircon/types.h>

#include <type_traits>

#include <object/pci_device_dispatcher.h>

#include "device.h"

namespace virtio {
// Get the virtio backend for a given pci virtio device.
zx::result<std::unique_ptr<virtio::Backend>> GetBackend(KernelHandle<PciDeviceDispatcher> pci,
                                                        zx_pcie_device_info_t info);

// Creates a Virtio device by determining the backend and moving that into
// |VirtioDevice|'s constructor, then call's the device's Init() method. The
// device's Init() is expected to call DdkAdd. On success, ownership of the device
// is released to devmgr.
template <class VirtioDevice, class = typename std::enable_if<
                                  std::is_base_of<virtio::Device, VirtioDevice>::value>::type>
zx_status_t CreateAndBind(void* /*ctx*/, KernelHandle<PciDeviceDispatcher> device,
                          zx_pcie_device_info_t info) {
  auto backend = GetBackend(ktl::move(device), info);
  if (!backend.is_ok()) {
    return backend.status_value();
  }
  fbl::AllocChecker ac;
  auto dev = ktl::make_unique<VirtioDevice>(&ac, std::move(backend.value()));
  if (!ac.check()) {
    return ZX_ERR_NO_MEMORY;
  }
  zx_status_t status = dev->Init();
  if (status == ZX_OK) {
    // devmgr is now in charge of the memory for dev
    [[maybe_unused]] auto ptr = dev.release();
  }
  return status;
}

}  // namespace virtio
#endif  // SRC_DEVICES_BUS_LIB_VIRTIO_INCLUDE_LIB_VIRTIO_DRIVER_UTILS_H_
