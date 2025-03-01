// Copyright 2016 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <assert.h>
#include <inttypes.h>
#include <lib/virtio/backends/pci.h>
#include <lib/virtio/device.h>
#include <limits.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <trace.h>
#include <zircon/types.h>

#include <utility>

#include <pretty/hexdump.h>

#define LOCAL_TRACE 0

namespace virtio {

Device::Device(ktl::unique_ptr<Backend> backend) : backend_(std::move(backend)) {}

Device::~Device() { TRACEF("%s: exit", __func__); }

void Device::Release() {
  backend_->Terminate();
  irq_thread_should_exit_.store(true, std::memory_order_release);
  irq_thread_->Join(nullptr, ZX_TIME_INFINITE);
  backend_.reset();
}

void Device::IrqWorker() {
  const auto irq_mode = backend_->InterruptMode();
  ZX_DEBUG_ASSERT(irq_mode == PCIE_IRQ_MODE_LEGACY || irq_mode == PCIE_IRQ_MODE_MSI_X);
  TRACEF("starting %s irq worker", (irq_mode == PCIE_IRQ_MODE_LEGACY) ? "legacy" : "msi-x");

  while (backend_->InterruptValid() == ZX_OK) {
    auto result = backend_->WaitForInterrupt();
    if (!result.is_ok()) {
      if (result.status_value() != ZX_ERR_TIMED_OUT) {
        LTRACEF_LEVEL(2, "error while waiting for interrupt: %d", result.error_value());
        break;
      }

      if (irq_thread_should_exit_.load(std::memory_order_relaxed)) {
        LTRACEF_LEVEL(2, "terminating irq thread");
        break;
      }

      // Timeouts are fine, but need to continue because there's nothing to ack.
      continue;
    }

    // Ack the interrupt we saw based on the key returned from the port. For legacy interrupts
    // this will always be 0, but MSI-X will depend on the number of vectors configured.
    auto key = result.value();
    backend_->InterruptAck(key);

    // Read the status before completing the interrupt in case
    // another interrupt fires and changes the status.
    if (irq_mode == PCIE_IRQ_MODE_LEGACY) {
      uint32_t irq_status = IsrStatus();
      LTRACEF_LEVEL(2, "irq_status: %#x\n", irq_status);

      // Since we handle both interrupt types here it's possible for a
      // spurious interrupt if they come in sequence and we check IsrStatus
      // after both have been triggered.
      if (irq_status) {
        if (irq_status & VIRTIO_ISR_QUEUE_INT) { /* used ring update */
          IrqRingUpdate();
        }
        if (irq_status & VIRTIO_ISR_DEV_CFG_INT) { /* config change */
          IrqConfigChange();
        }
      }
    } else {
      // MSI-X
      LTRACEF_LEVEL(2, "irq key: %u\n", key);
      switch (key) {
        case PciBackend::kMsiConfigVector:
          IrqConfigChange();
          break;
        case PciBackend::kMsiQueueVector:
          IrqRingUpdate();
          break;
      }
    }
  }
}

int Device::IrqThreadEntry(void* arg) {
  Device* d = static_cast<Device*>(arg);
  d->IrqWorker();
  return 0;
}

void Device::StartIrqThread() {
  std::array<char, ZX_MAX_NAME_LEN> name{};
  snprintf(name.data(), name.size(), "%s-irq-worker", tag());
  // thrd_create_with_name(&irq_thread_, IrqThreadEntry, this, name.data());
  irq_thread_ = Thread::Create(name.data(), IrqThreadEntry, this, DEFAULT_PRIORITY);
  irq_thread_->Resume();
}

void Device::CopyDeviceConfig(void* _buf, size_t len) const {
  assert(_buf);

  for (size_t i = 0; i < len; i++) {
    backend_->ReadDeviceConfig(static_cast<uint16_t>(i), static_cast<uint8_t*>(_buf) + i);
  }
}

}  // namespace virtio
