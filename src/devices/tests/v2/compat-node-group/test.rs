// Copyright 2022 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use anyhow::{anyhow, Error, Result};
use fuchsia_component::server::ServiceFs;
use fuchsia_component_test::{ChildOptions, LocalComponentHandles, RealmBuilder};
use fuchsia_driver_test::{DriverTestRealmBuilder, DriverTestRealmInstance};
use futures::channel::mpsc;
use futures::{StreamExt, TryStreamExt};
use {
    fidl_fuchsia_compat_nodegroup_test as fcdt, fidl_fuchsia_driver_test as fdt,
    fuchsia_async as fasync,
};

const WAITER_NAME: &'static str = "waiter";

async fn waiter_serve(mut stream: fcdt::WaiterRequestStream, mut sender: mpsc::Sender<()>) {
    while let Some(fcdt::WaiterRequest::Ack { status, .. }) =
        stream.try_next().await.expect("Stream failed")
    {
        assert_eq!(status, zx::Status::OK.into_raw());
        sender.try_send(()).expect("Sender failed")
    }
}

async fn waiter_component(
    handles: LocalComponentHandles,
    sender: mpsc::Sender<()>,
) -> Result<(), Error> {
    let mut fs = ServiceFs::new();
    fs.dir("svc").add_fidl_service(move |stream: fcdt::WaiterRequestStream| {
        fasync::Task::spawn(waiter_serve(stream, sender.clone())).detach()
    });
    fs.serve_connection(handles.outgoing_dir)?;
    Ok(fs.collect::<()>().await)
}

#[fasync::run_singlethreaded(test)]
async fn test_compat_nodegroup() -> Result<()> {
    let (sender, mut receiver) = mpsc::channel(1);

    // Create the RealmBuilder.
    let builder = RealmBuilder::new().await?;
    builder.driver_test_realm_setup().await?;
    let waiter = builder
        .add_local_child(
            WAITER_NAME,
            move |handles: LocalComponentHandles| {
                Box::pin(waiter_component(handles, sender.clone()))
            },
            ChildOptions::new(),
        )
        .await?;

    let offer = fuchsia_component_test::Capability::protocol::<fcdt::WaiterMarker>().into();
    let dtr_offers = vec![offer];

    builder.driver_test_realm_add_dtr_offers(&dtr_offers, (&waiter).into()).await?;

    let instance = builder.build().await?;

    // Start the DriverTestRealm.
    let args = fdt::RealmArgs {
        root_driver: Some("fuchsia-boot:///dtr#meta/test-parent-sys.cm".to_string()),
        dtr_offers: Some(dtr_offers),
        ..Default::default()
    };
    instance.driver_test_realm_start(args).await?;

    receiver.next().await.ok_or(anyhow!("Receiver failed"))?;
    Ok(())
}
