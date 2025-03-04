// Copyright 2021 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

#include <fuchsia/wlan/ieee80211/c/banjo.h>
#include <zircon/errors.h>

#include <zxtest/zxtest.h>

#include "src/connectivity/wlan/drivers/third_party/broadcom/brcmfmac/sim/test/sim_test.h"
#include "src/connectivity/wlan/lib/common/cpp/include/wlan/common/macaddr.h"

namespace wlan::brcmfmac {
namespace {

constexpr zx::duration kSimulatedClockDuration = zx::sec(10);

}  // namespace

const common::MacAddr kDefaultMac({0x12, 0x34, 0x56, 0x65, 0x43, 0x21});

// Verify that a query for MAC sublayer features support works on a client interface
TEST_F(SimTest, ClientIfcQueryMacSublayerSupport) {
  ASSERT_EQ(Init(), ZX_OK);

  SimInterface client_ifc;
  ASSERT_EQ(StartInterface(wlan_common::WlanMacRole::kClient, &client_ifc, kDefaultMac), ZX_OK);

  wlan_common::MacSublayerSupport resp;
  env_->ScheduleNotification(std::bind(&SimInterface::QueryMacSublayerSupport, &client_ifc, &resp),
                             zx::sec(1));
  env_->Run(kSimulatedClockDuration);

  EXPECT_FALSE(resp.rate_selection_offload.supported);
  EXPECT_EQ(resp.data_plane.data_plane_type, wlan_common::DataPlaneType::kGenericNetworkDevice);
  EXPECT_EQ(resp.device.mac_implementation_type, wlan_common::MacImplementationType::kFullmac);
}

// Verify that a query for security features support works on a client interface
TEST_F(SimTest, ClientIfcQuerySecuritySupport) {
  ASSERT_EQ(Init(), ZX_OK);

  SimInterface client_ifc;
  ASSERT_EQ(StartInterface(wlan_common::WlanMacRole::kClient, &client_ifc, kDefaultMac), ZX_OK);

  wlan_common::SecuritySupport resp;
  env_->ScheduleNotification(std::bind(&SimInterface::QuerySecuritySupport, &client_ifc, &resp),
                             zx::sec(1));
  env_->Run(kSimulatedClockDuration);

  EXPECT_FALSE(resp.sae.driver_handler_supported);
  EXPECT_TRUE(resp.sae.sme_handler_supported);
  EXPECT_TRUE(resp.mfp.supported);
}

// Verify that a query for spectrum management features support works on a client interface
TEST_F(SimTest, ClientIfcQuerySpectrumManagementSupport) {
  ASSERT_EQ(Init(), ZX_OK);

  SimInterface client_ifc;
  ASSERT_EQ(StartInterface(wlan_common::WlanMacRole::kClient, &client_ifc, kDefaultMac), ZX_OK);

  wlan_common::SpectrumManagementSupport resp;
  env_->ScheduleNotification(
      std::bind(&SimInterface::QuerySpectrumManagementSupport, &client_ifc, &resp), zx::sec(1));
  env_->Run(kSimulatedClockDuration);

  EXPECT_TRUE(resp.dfs.supported);
}

}  // namespace wlan::brcmfmac
