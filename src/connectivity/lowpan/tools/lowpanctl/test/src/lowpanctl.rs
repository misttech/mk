// Copyright 2019 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use anyhow::{bail, format_err, Context as _, Error};
use fasync::MonotonicInstant;
use fidl_fuchsia_lowpan::DeviceWatcherMarker;
use fuchsia_async as fasync;
use fuchsia_async::TimeoutExt;
use fuchsia_component::client::connect_to_protocol;
use fuchsia_component_test::ScopedInstanceFactory;
use futures::prelude::*;

const DEFAULT_TIMEOUT: zx::MonotonicDuration = zx::MonotonicDuration::from_seconds(50);

#[fasync::run_singlethreaded(test)]
pub async fn test_lowpanctl() {
    test_lowpanctl_status().await;
    test_lowpanctl_leave().await;
    test_lowpanctl_reset().await;
    test_lowpanctl_list().await;
    test_lowpanctl_mfg().await;
    test_lowpanctl_provision().await;
    test_lowpanctl_join_network().await;
    test_lowpanctl_form_network().await;
    test_lowpanctl_energy_scan().await;
    test_lowpanctl_network_scan().await;
    test_lowpanctl_set_active().await;
    test_lowpanctl_get_credential().await;
    test_lowpanctl_get_supported_channels().await;
    test_lowpanctl_get_supported_network_types().await;
    test_lowpanctl_get_mac_filter_settings().await;
    test_lowpanctl_replace_mac_filter_settings().await;
    test_lowpanctl_get_neighbor_table().await;
    test_lowpanctl_get_counters().await;
    test_lowpanctl_reset_counters().await;
}

pub async fn test_lowpanctl_status() {
    test_lowpanctl_command(vec!["status".to_string()])
        .await
        .expect("Call to `lowpanctl status` failed.");

    test_lowpanctl_command(vec!["status".to_string(), "--format".to_string(), "csv".to_string()])
        .await
        .expect("Call to `lowpanctl status --format csv` failed.");

    test_lowpanctl_command(vec!["status".to_string(), "--format".to_string(), "std".to_string()])
        .await
        .expect("Call to `lowpanctl status --format std` failed.");
}

pub async fn test_lowpanctl_leave() {
    test_lowpanctl_command(vec!["leave".to_string()])
        .await
        .expect("Call to `lowpanctl leave` failed.");
}

pub async fn test_lowpanctl_reset() {
    test_lowpanctl_command(vec!["reset".to_string()])
        .await
        .expect("Call to `lowpanctl reset` failed.");
}

pub async fn test_lowpanctl_list() {
    test_lowpanctl_command(vec!["list".to_string()])
        .await
        .expect("Call to `lowpanctl list` failed.");
}

pub async fn test_lowpanctl_mfg() {
    test_lowpanctl_command(vec!["mfg".to_string(), "help".to_string()])
        .await
        .expect("Call to `lowpanctl mfg help` failed.");
}

pub async fn test_lowpanctl_provision() {
    test_lowpanctl_command(vec![
        "provision".to_string(),
        "--name".to_string(),
        "some_name".to_string(),
    ])
    .await
    .expect("Call to `lowpanctl provision` failed.");
}

pub async fn test_lowpanctl_energy_scan() {
    test_lowpanctl_command(vec![
        "energy-scan".to_string(),
        "--channels".to_string(),
        "10,100,1000,10000".to_string(),
        "--dwell-time-ms".to_string(),
        "1234567890".to_string(),
    ])
    .await
    .expect("Call to `lowpanctl energy-scan` failed.");
}

pub async fn test_lowpanctl_network_scan() {
    test_lowpanctl_command(vec![
        "network-scan".to_string(),
        "--channels".to_string(),
        "5,50,500,5000".to_string(),
        "--tx-power-dbm".to_string(),
        "-100".to_string(),
    ])
    .await
    .expect("Call to `lowpanctl energy-scan` failed.");
}

pub async fn test_lowpanctl_join_network() {
    test_lowpanctl_command(vec!["join".to_string(), "--name".to_string(), "some_name".to_string()])
        .await
        .expect("Call to `lowpanctl join` failed.");
}

pub async fn test_lowpanctl_form_network() {
    test_lowpanctl_command(vec!["form".to_string(), "--name".to_string(), "some_name".to_string()])
        .await
        .expect("Call to `lowpanctl form` failed.");
}

pub async fn test_lowpanctl_set_active() {
    test_lowpanctl_command(vec!["set-active".to_string(), "true".to_string()])
        .await
        .expect("Call to `lowpanctl set-active` failed.");
}

pub async fn test_lowpanctl_get_credential() {
    test_lowpanctl_command(vec!["get-credential".to_string()])
        .await
        .expect("Call to `lowpanctl get-credential` failed.");
}

pub async fn test_lowpanctl_get_supported_channels() {
    test_lowpanctl_command(vec!["get-supported-channels".to_string()])
        .await
        .expect("Call to `lowpanctl get-supported-channels` failed.");
}

pub async fn test_lowpanctl_get_supported_network_types() {
    test_lowpanctl_command(vec!["get-supported-network-types".to_string()])
        .await
        .expect("Call to `lowpanctl get-supported-network-types` failed.");
}

pub async fn test_lowpanctl_get_mac_filter_settings() {
    test_lowpanctl_command(vec!["get-mac-filter-settings".to_string()])
        .await
        .expect("Call to `lowpanctl get-supported-network-types` failed.");
}

pub async fn test_lowpanctl_replace_mac_filter_settings() {
    test_lowpanctl_command(vec![
        "replace-mac-filter-settings".to_string(),
        "--mode".to_string(),
        "disabled".to_string(),
    ])
    .await
    .expect("Call to `lowpanctl replace-supported-network-types` failed.");
    test_lowpanctl_command(vec![
        "replace-mac-filter-settings".to_string(),
        "--mode".to_string(),
        "allow".to_string(),
        "--mac-addr-filter-items".to_string(),
        "7AAD966C55768C64".to_string(),
    ])
    .await
    .expect("Call to `lowpanctl replace-supported-network-types` failed.");
    test_lowpanctl_command(vec![
        "replace-mac-filter-settings".to_string(),
        "--mode".to_string(),
        "allow".to_string(),
        "--mac-addr-filter-items".to_string(),
        "7AAD966C55768C64.-10,7AAD966C55768C65.-20".to_string(),
    ])
    .await
    .expect("Call to `lowpanctl replace-supported-network-types` failed.");
    test_lowpanctl_command(vec![
        "replace-mac-filter-settings".to_string(),
        "--mode".to_string(),
        "deny".to_string(),
        "--mac-addr-filter-items".to_string(),
        "7AAD966C55768C64".to_string(),
    ])
    .await
    .expect("Call to `lowpanctl replace-supported-network-types` failed.");
}

pub async fn test_lowpanctl_get_neighbor_table() {
    test_lowpanctl_command(vec!["get-neighbor-table".to_string()])
        .await
        .expect("Call to `lowpanctl get-neighbor-table` failed.");
}

pub async fn test_lowpanctl_get_counters() {
    test_lowpanctl_command(vec!["get-counters".to_string()])
        .await
        .expect("Call to `lowpanctl get-counters` failed.");
}

pub async fn test_lowpanctl_reset_counters() {
    test_lowpanctl_command(vec!["get-counters".to_string(), "--reset".to_string()])
        .await
        .expect("Call to `lowpanctl get-counters --reset` failed.");
}

pub async fn test_lowpanctl_command(args: Vec<String>) -> Result<(), Error> {
    // Step 1: Get an instance of the DeviceWatcher API and make sure there are no devices registered.
    let lookup = connect_to_protocol::<DeviceWatcherMarker>()
        .context("Failed to connect to Lowpan DeviceWatcher service")?;

    let (added, removed) = lookup
        .watch_devices()
        .err_into::<Error>()
        .on_timeout(MonotonicInstant::after(DEFAULT_TIMEOUT), || {
            Err(format_err!("Timeout waiting for lookup.watch_devices()"))
        })
        .await
        .context("Initial call to lookup.watch_devices() failed")?;

    assert!(added.is_empty(), "Initial device list not empty");
    assert!(removed.is_empty(), "Initial device watch had removed devices");

    // Step 2: Start a LoWPAN Dummy Driver
    println!("Starting lowpan dummy driver");
    let dummy_driver = ScopedInstanceFactory::new("drivers")
        .new_instance("#meta/lowpan-dummy-driver.cm")
        .await
        .context("creating lowpan dummy driver")?;
    dummy_driver.connect_to_binder()?;

    // Step 3: Wait to receive an event that the driver has registered.
    let (added, removed) = lookup
        .watch_devices()
        .err_into::<Error>()
        .on_timeout(MonotonicInstant::after(DEFAULT_TIMEOUT), || {
            Err(format_err!("Timeout waiting for lookup.watch_devices()"))
        })
        .await
        .context("Second call to lookup.watch_devices() failed")?;

    assert_eq!(added, vec!["lowpan0".to_string()]);
    assert!(removed.is_empty(), "Second device watch had removed devices");

    // Step 4: Call lowpanctl
    println!("Calling lowpanctl with {:?}", args);
    let output = std::process::Command::new("/pkg/bin/lowpanctl")
        .args(args)
        .output()
        .context("waiting for lowpanctl to finish")?;
    if !output.status.success() {
        bail!("lowpanctl command failed: {:#?}", output);
    }

    println!("Command \"lowpanctl\" completed");

    // Step 5: Kill the dummy driver.
    drop(dummy_driver);

    // Step 6: Wait to receive an event that the driver has unregistered.
    let (added, removed) = lookup
        .watch_devices()
        .err_into::<Error>()
        .on_timeout(MonotonicInstant::after(DEFAULT_TIMEOUT), || {
            Err(format_err!("Timeout waiting for lookup.watch_devices()"))
        })
        .await
        .context("Final call to lookup.watch_devices() failed")?;

    assert!(added.is_empty(), "Final device watch had added devices");
    assert_eq!(removed, vec!["lowpan0".to_string()]);

    Ok(())
}
