// Copyright 2020 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::message::action_fuse::{ActionFuse, ActionFuseHandle};
use crate::message::base::{Message, MessageClientId, MessageEvent, MessengerId, Status};
use crate::message::message_client::MessageClient;
use crate::message::messenger::Messenger;
use crate::message::receptor::Receptor;
use anyhow::{format_err, Error};
use fuchsia_async::{self as fasync, DurationExt};
use futures::channel::mpsc::UnboundedSender;
use futures::future::{AbortHandle, Abortable, TryFutureExt};
use futures::lock::Mutex;
use std::rc::Rc;
use zx::MonotonicDuration;

/// Helper for creating a beacon. The builder allows chaining additional fuses
pub struct BeaconBuilder {
    messenger: Messenger,
    chained_fuses: Option<ActionFuseHandle>,
    timeout: Option<MonotonicDuration>,
}

impl BeaconBuilder {
    pub(super) fn new(messenger: Messenger) -> Self {
        Self { messenger, chained_fuses: None, timeout: None }
    }

    pub(super) fn add_fuse(mut self, fuse: ActionFuseHandle) -> Self {
        self.chained_fuses = Some(fuse);
        self
    }

    pub(super) fn set_timeout(mut self, duration: Option<MonotonicDuration>) -> Self {
        self.timeout = duration;
        self
    }

    pub(super) fn build(self) -> (Beacon, Receptor) {
        Beacon::create(self.messenger, self.chained_fuses, self.timeout)
    }
}

/// A Beacon is the conduit for sending messages to a particular Receptor. An
/// instance may be cloned and passed around to other components. All copies of
/// a particular Beacon share a reference to an flag that signals whether the
/// Receptor is active, which controls whether future messages will be sent.
///
/// It is important to note that Beacons spawn from sending a Message. Status
/// and other context sent through the Beacon are in relation to this original
/// Message (either an origin or reply).
#[derive(Clone, Debug)]
pub struct Beacon {
    /// A reference to the associated Messenger. This is only used when delivering
    /// a new message to a beacon, where a MessageClient (which references both
    /// the recipient's Messenger and the message) must be created.
    messenger: Messenger,
    /// The sender half of an internal channel established between the Beacon and
    /// Receptor.
    event_sender: UnboundedSender<MessageEvent>,
    /// Sentinel for secondary ActionFuses
    sentinel: Rc<Mutex<Sentinel>>,
    /// Timeout for firing if a response payload is not delivered in time.
    timeout_abort_client: AbortHandle,
}

impl Beacon {
    /// Creates a Beacon, Receptor tuple. The Messenger provided as an argument
    /// will be associated with any delivered Message for reply purposes.
    fn create(
        messenger: Messenger,
        fuses: Option<ActionFuseHandle>,
        timeout: Option<MonotonicDuration>,
    ) -> (Beacon, Receptor) {
        let sentinel = Rc::new(Mutex::new(Sentinel::new()));
        let (event_tx, event_rx) = futures::channel::mpsc::unbounded::<MessageEvent>();
        let (timeout_abort_client, timeout_abort_server) = AbortHandle::new_pair();
        let signature = messenger.get_signature();
        let beacon = Beacon {
            messenger,
            event_sender: event_tx.clone(),
            sentinel: sentinel.clone(),
            timeout_abort_client: timeout_abort_client.clone(),
        };

        // pass fuse to receptor to hold and set when it goes out of scope.
        let receptor = Receptor::new(
            signature,
            event_rx,
            ActionFuse::create(Box::new(move || {
                let sentinel = sentinel.clone();
                fasync::Task::local(async move {
                    timeout_abort_client.abort();
                    sentinel.lock().await.trigger().await;
                })
                .detach();
            })),
            fuses,
        );

        if let Some(duration) = timeout {
            let abortable_timeout = Abortable::new(
                async move {
                    fuchsia_async::Timer::new(duration.after_now()).await;
                    // Panic if send failed, otherwise the client cannot abort processes.
                    event_tx
                        .unbounded_send(MessageEvent::Status(Status::Timeout))
                        .expect("Beacon::create, event_tx failed to send Timeout status message");
                },
                timeout_abort_server,
            );

            fasync::Task::local(abortable_timeout.unwrap_or_else(|_| ())).detach();
        }
        (beacon, receptor)
    }

    /// Sends the Status associated with the original message that spawned
    /// this beacon.
    pub(super) async fn status(&self, status: Status) -> Result<(), Error> {
        if self.event_sender.unbounded_send(MessageEvent::Status(status)).is_err() {
            return Err(format_err!("failed to deliver status"));
        }

        Ok(())
    }

    /// Delivers a response to the original message that spawned this Beacon.
    pub(super) async fn deliver(
        &self,
        message: Message,
        client_id: MessageClientId,
    ) -> Result<(), Error> {
        self.timeout_abort_client.abort();
        if self
            .event_sender
            .unbounded_send(MessageEvent::Message(
                message.payload().clone(),
                MessageClient::new(client_id, message, self.messenger.clone()),
            ))
            .is_err()
        {
            return Err(format_err!("failed to deliver message"));
        }

        Ok(())
    }

    /// Adds the specified fuse to the beacon's sentinel.
    pub(super) async fn add_fuse(&mut self, fuse: ActionFuseHandle) {
        self.sentinel.lock().await.add_fuse(fuse);
    }

    /// Returns the identifier for the associated Messenger.
    pub(super) fn get_messenger_id(&self) -> MessengerId {
        self.messenger.get_id()
    }
}

/// Sentinel gathers actions fuses from other sources and releases them
/// on-demand.
struct Sentinel {
    active: bool,
    fuses: Vec<ActionFuseHandle>,
}

impl Sentinel {
    /// Generates a new Sentinel.
    fn new() -> Self {
        Self { active: true, fuses: vec![] }
    }

    /// Adds a fuse if still active.
    fn add_fuse(&mut self, fuse: ActionFuseHandle) {
        // In the case we're not active anymore, do not add fuse.
        if !self.active {
            return;
        }

        self.fuses.push(fuse);
    }

    /// Removes all pending fuses.
    async fn trigger(&mut self) {
        self.active = false;
        // Clear fuses, triggering them.
        self.fuses.clear();
    }
}
