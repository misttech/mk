// Copyright 2020 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![warn(missing_docs)]

//! `time_test_client` is a simple component that logs diagnostic information about the time it
//! receives from the system to aid in debugging and (potentially in the future) automated testing.

use anyhow::Error;
use chrono::{DateTime, TimeZone as _, Timelike as _, Utc};
use futures::prelude::*;
use lazy_static::lazy_static;
use log::{info, warn};
use {fuchsia_async as fasync, fuchsia_runtime as runtime};

/// Delay between polls of system and userspace clocks.
const POLL_DELAY: zx::MonotonicDuration = zx::MonotonicDuration::from_seconds(2);

lazy_static! {
   /// Rights to request when duplicating handles to userspace clocks.
   static ref CLOCK_RIGHTS: zx::Rights = zx::Rights::READ | zx::Rights::DUPLICATE | zx::Rights::WAIT;
}

#[fasync::run_singlethreaded]
async fn main() {
    let mut futures = vec![];
    RuntimeUtcMonitor::new();
    match ClockMonitor::new() {
        Ok(clock_monitor) => futures.push(clock_monitor.execute().boxed()),
        Err(err) => warn!("{}", err),
    }
    future::join_all(futures).await;
}

/// Returns a standard format timestamp for a zx::MonotonicInstant.
fn zx_timestamp(time: &zx::MonotonicInstant) -> String {
    chrono_timestamp(&Utc.timestamp_nanos(time.into_nanos()))
}

/// Returns a standard format timestamp for a chrono::DateTime.
fn chrono_timestamp(time: &DateTime<Utc>) -> String {
    (&time.to_rfc3339()[0..23]).to_string()
}

/// A monitor for UTC as reported by the runtime.
struct RuntimeUtcMonitor {
    /// The UTC time when this monitor was initialized.
    initial: DateTime<Utc>,
}

impl RuntimeUtcMonitor {
    /// Creates a new `RuntimeUtcMonitor`, logging the initial state.
    pub fn new() -> Self {
        let initial = Utc::now();
        info!("Runtime UTC at initialization: {}", chrono_timestamp(&initial));
        RuntimeUtcMonitor { initial }
    }

    /// Async function to operate this monitor. Currently unused because we don't need to dump both
    /// the runtime and kernel clocks every minute, but when the Kernel clock goes away we'll need
    /// this again.
    #[allow(unused)]
    async fn execute(self) {
        let mut last_logged = self.initial;
        loop {
            fasync::Timer::new(fasync::MonotonicInstant::after(POLL_DELAY)).await;
            // Only log UTC when we reach a new minute.
            let current = Utc::now();
            if current.hour() != last_logged.hour() || current.minute() != last_logged.minute() {
                info!("Runtime UTC: {}", chrono_timestamp(&current));
                last_logged = current;
            }
        }
    }
}

/// A monitor for a UTC `zx::Clock` to log changes in clock details.
struct ClockMonitor {
    /// The `zx::Clock` to be monitored.
    clock: zx::Clock,
    /// The generation counter that was present during initialization.
    initial_generation: u32,
}

impl ClockMonitor {
    /// Creates a new `ClockMonitor`, logging the initial state, or returns an error if a clock
    /// could not be found.
    pub fn new() -> Result<Self, Error> {
        // Retrieve a handle to the UTC clock.
        let clock = runtime::duplicate_utc_clock_handle(*CLOCK_RIGHTS)
            .map_err(|stat| anyhow::anyhow!("Error retreiving userspace UTC handle: {}", stat))?;

        // Log the time reported by the clock.
        match clock.read() {
            Ok(time) => info!("Userspace UTC at initialization: {}", zx_timestamp(&time)),
            Err(stat) => warn!("Error reading userspace UTC initial time {}", stat),
        }

        // Log the initial details and backstop time on creation
        match clock.get_details() {
            Ok(details) => {
                info!("Userspace UTC backstop time: {}", zx_timestamp(&details.backstop));
                info!(
                    "Userspace UTC details at initialization: {}",
                    Self::describe_clock_details(&details)
                );
                Ok(ClockMonitor { clock, initial_generation: details.generation_counter })
            }
            Err(stat) => {
                warn!("Error reading userspace UTC details {}", stat);
                Ok(ClockMonitor { clock, initial_generation: 0 })
            }
        }
    }

    /// Async function to operate this monitor.
    async fn execute(self) {
        // Future to log when the clock started signal is observed.
        let clock_start_fut = async {
            match fasync::OnSignals::new(&self.clock, zx::Signals::CLOCK_STARTED).await {
                Ok(_) => info!("Userspace UTC has started"),
                Err(err) => warn!("Error waiting for userspace UTC start: {}", err),
            }
        };

        // Future to log every time a new generation_counter is seen (note we don't print errors on
        // failing to read details, such a failure is likely persistent and would be spammy).
        let generation_change_fut = async {
            let mut last_logged = self.initial_generation;
            loop {
                fasync::Timer::new(fasync::MonotonicInstant::after(POLL_DELAY)).await;
                if let Ok(details) = self.clock.get_details() {
                    if details.generation_counter != last_logged {
                        info!(
                            "Updated userspace UTC details: {}",
                            Self::describe_clock_details(&details)
                        );
                        match self.clock.read() {
                            Ok(time) => info!("Updated userspace UTC: {}", zx_timestamp(&time)),
                            Err(stat) => warn!("Error reading userspace UTC after update {}", stat),
                        }
                        last_logged = details.generation_counter;
                    }
                }
            }
        };

        future::join(clock_start_fut, generation_change_fut).await;
    }

    /// Returns a string description of the most useful information in a `zx::ClockDetails`.
    fn describe_clock_details(clock_details: &zx::ClockDetails) -> String {
        let transform = &clock_details.mono_to_synthetic;
        format!(
            "gen={}, offset={:.1}s, rate={}/{}, err_bound={:.3}ms",
            clock_details.generation_counter,
            (transform.synthetic_offset - transform.reference_offset) as f64 / 1e9f64,
            transform.rate.synthetic_ticks,
            transform.rate.reference_ticks,
            (clock_details.error_bounds as f64) / 1e6f64,
        )
    }
}
