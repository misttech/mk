// Copyright 2025 Mist Tecnologia Ltda
// Copyright 2016 The Fuchsia Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

#include <align.h>
#include <platform.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <trace.h>
#include <zircon/errors.h>
#include <zircon/hw/pci.h>
#include <zircon/syscalls/pci.h>
#include <zircon/types.h>

#include <dev/bus/pci.h>
#include <lk/init.h>
#include <platform/pc/acpi.h>

#include <ktl/enforce.h>

#define LOCAL_TRACE 0

void kernel_pci_init(uint level) {
  const acpi_lite::AcpiMcfgTable* mcfg_table =
      acpi_lite::GetTableByType<acpi_lite::AcpiMcfgTable>(GlobalAcpiLiteParser());
  if (!mcfg_table) {
    dprintf(CRITICAL, "could not find MCFG\n");
    return;
  }

  const acpi_lite::AcpiMcfgAllocation* table_start =
      reinterpret_cast<const acpi_lite::AcpiMcfgAllocation*>(
          reinterpret_cast<uintptr_t>(mcfg_table) + sizeof(*mcfg_table));

  const acpi_lite::AcpiMcfgAllocation* table_end =
      reinterpret_cast<const acpi_lite::AcpiMcfgAllocation*>(
          reinterpret_cast<uintptr_t>(mcfg_table) + mcfg_table->size());

  uintptr_t table_bytes =
      reinterpret_cast<uintptr_t>(table_end) - reinterpret_cast<uintptr_t>(table_start);
  if (table_bytes % sizeof(*table_start) != 0) {
    dprintf(CRITICAL, "MCFG has unexpected size\n");
    return;
  }

  size_t num_entries = table_end - table_start;
  if (num_entries == 0) {
    dprintf(CRITICAL, "MCFG has no entries\n");
    return;
  }

  if (num_entries > 1) {
    dprintf(INFO, "MCFG has more than one entry, just taking the first\n");
  }

  dprintf(INFO, "PCI MCFG: segment %#hx bus [%hhu...%hhu] address %#lx\n", table_start->segment,
          table_start->start_bus, table_start->end_bus, table_start->base_address);

  // try to initialize pci based on the MCFG ecam aperture
  status_t err = pci_init_ecam(table_start->base_address, table_start->segment,
                               table_start->start_bus, table_start->end_bus);
  if (err == NO_ERROR) {
    pci_bus_mgr_init();
  }
}

LK_INIT_HOOK(x86_pcie_bus, kernel_pci_init, LK_INIT_LEVEL_PLATFORM)
