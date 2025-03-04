// Copyright 2021 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::config::StreamSink;
use async_trait::async_trait;
use cm_logger::scoped::ScopedLogger;
use cm_types::NamespacePath;
use fidl::prelude::*;
use fuchsia_component::client::connect_to_named_protocol_at_dir_root;
use fuchsia_runtime::{HandleInfo, HandleType};
use futures::StreamExt;
use lazy_static::lazy_static;
use log::warn;
use namespace::Namespace;
use once_cell::unsync::OnceCell;
use socket_parsing::{NewlineChunker, NewlineChunkerError};
use std::sync::Arc;
use zx::HandleBased;
use {fidl_fuchsia_logger as flogger, fidl_fuchsia_process as fproc, fuchsia_async as fasync};

const STDOUT_FD: i32 = 1;
const STDERR_FD: i32 = 2;

lazy_static! {
    static ref SVC_DIRECTORY_PATH: NamespacePath = "/svc".parse().unwrap();
}

/// Max size for message when draining input stream socket. This number is
/// slightly smaller than size allowed by Archivist (LogSink service implementation).
const MAX_MESSAGE_SIZE: usize = 30720;

/// Bind stdout or stderr streams to syslog. This function binds either or both
/// output streams to syslog depending on value provided for each streams'
/// StreamSink. If the value for an output stream is set to StreamSink::Log,
/// that stream's file descriptor will be bound to syslog. All writes on that
// fd will be forwarded to syslog and will register as log entries. For stdout,
// the messages will be tagged with severity INFO. For stderr, the messages
// will be tagged with severity WARN. A task is created to listen to writes on
// the appropriate file descriptor and forward the message to syslog. This
// function returns both the task for each file descriptor and its
// corresponding HandleInfo.
pub fn bind_streams_to_syslog(
    ns: &Namespace,
    stdout_sink: StreamSink,
    stderr_sink: StreamSink,
) -> (Vec<fasync::Task<()>>, Vec<fproc::HandleInfo>) {
    let mut tasks: Vec<fasync::Task<()>> = Vec::new();
    let mut handles: Vec<fproc::HandleInfo> = Vec::new();

    // connect to the namespace's logger if we'll need it, wrap in OnceCell so we only do it once
    // (can't use Lazy here because we need to capture `ns`)
    let logger = OnceCell::new();
    let mut forward_stream = |sink, fd, level| {
        if matches!(sink, StreamSink::Log) {
            // create the handle before dealing with the logger so components still receive an inert
            // handle if connecting to LogSink fails
            let (socket, handle_info) =
                new_socket_bound_to_fd(fd).expect("failed to create socket");
            handles.push(handle_info);

            if let Some(l) = logger.get_or_init(|| create_namespace_logger(ns).map(Arc::new)) {
                tasks.push(forward_socket_to_syslog(l.clone(), socket, level));
            } else {
                warn!("Tried forwarding file descriptor {fd} but didn't have a LogSink available.");
            }
        }
    };

    forward_stream(stdout_sink, STDOUT_FD, OutputLevel::Info);
    forward_stream(stderr_sink, STDERR_FD, OutputLevel::Warn);

    (tasks, handles)
}

fn create_namespace_logger(ns: &Namespace) -> Option<ScopedLogger> {
    let svc_dir = ns.get(&SVC_DIRECTORY_PATH)?;
    let logsink = connect_to_named_protocol_at_dir_root::<flogger::LogSinkMarker>(
        svc_dir,
        flogger::LogSinkMarker::PROTOCOL_NAME,
    )
    .ok()?;
    ScopedLogger::create(logsink).ok()
}

fn forward_socket_to_syslog(
    logger: Arc<ScopedLogger>,
    socket: fasync::Socket,
    level: OutputLevel,
) -> fasync::Task<()> {
    let mut writer = SyslogWriter::new(logger, level);
    let task = fasync::Task::spawn(async move {
        if let Err(error) = drain_lines(socket, &mut writer).await {
            warn!(error:%; "Draining output stream failed");
        }
    });

    task
}

fn new_socket_bound_to_fd(fd: i32) -> Result<(fasync::Socket, fproc::HandleInfo), zx::Status> {
    let (tx, rx) = zx::Socket::create_stream();
    let rx = fasync::Socket::from_socket(rx);
    Ok((
        rx,
        fproc::HandleInfo {
            handle: tx.into_handle(),
            id: HandleInfo::new(HandleType::FileDescriptor, fd as u16).as_raw(),
        },
    ))
}

/// Drains all bytes from socket and writes messages to writer. Bytes read
/// are split into lines and separated into chunks no greater than
/// MAX_MESSAGE_SIZE.
async fn drain_lines(
    socket: fasync::Socket,
    writer: &mut dyn LogWriter,
) -> Result<(), NewlineChunkerError> {
    let chunker = NewlineChunker::new(socket, MAX_MESSAGE_SIZE);
    futures::pin_mut!(chunker);

    while let Some(chunk_or_line) = chunker.next().await {
        writer.write(&chunk_or_line?).await;
    }

    Ok(())
}

/// Object capable of writing a stream of bytes.
#[async_trait]
trait LogWriter: Send {
    async fn write(&mut self, bytes: &[u8]);
}

struct SyslogWriter {
    logger: Arc<dyn log::Log + Send + Sync>,
    level: OutputLevel,
}

#[derive(Copy, Clone)]
enum OutputLevel {
    Info,
    Warn,
}

impl From<OutputLevel> for log::Level {
    fn from(level: OutputLevel) -> log::Level {
        match level {
            OutputLevel::Info => log::Level::Info,
            OutputLevel::Warn => log::Level::Warn,
        }
    }
}

impl SyslogWriter {
    fn new(logger: Arc<dyn log::Log + Send + Sync>, level: OutputLevel) -> Self {
        Self { logger, level }
    }
}

#[async_trait]
impl LogWriter for SyslogWriter {
    async fn write(&mut self, bytes: &[u8]) {
        let msg = String::from_utf8_lossy(&bytes);
        self.logger.log(
            &log::Record::builder().level(self.level.into()).args(format_args!("{msg}")).build(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{create_fs_with_mock_logsink, MockServiceFs, MockServiceRequest};
    use anyhow::{anyhow, format_err, Context, Error};
    use async_trait::async_trait;
    use diagnostics_message::MonikerWithUrl;
    use fidl_fuchsia_component_runner as fcrunner;
    use fidl_fuchsia_logger::LogSinkRequest;
    use fuchsia_async::Task;
    use futures::channel::mpsc;
    use futures::{try_join, FutureExt, SinkExt};
    use rand::distributions::{Alphanumeric, DistString as _};
    use rand::thread_rng;
    use std::sync::Mutex;

    #[async_trait]
    impl LogWriter for mpsc::Sender<String> {
        async fn write(&mut self, bytes: &[u8]) {
            let message =
                std::str::from_utf8(&bytes).expect("Failed to decode bytes to utf8.").to_owned();
            let () =
                self.send(message).await.expect("Failed to send message to other end of mpsc.");
        }
    }

    #[fuchsia::test]
    async fn syslog_writer_decodes_valid_utf8_message() -> Result<(), Error> {
        let (dir, ns_entries) = create_fs_with_mock_logsink()?;

        let ((), actual) = try_join!(
            write_to_syslog_or_panic(ns_entries, b"Hello World!"),
            read_message_from_syslog(dir)
        )?;

        assert_eq!(actual, Some("Hello World!".to_owned()));
        Ok(())
    }

    #[fuchsia::test]
    async fn syslog_writer_decodes_non_utf8_message() -> Result<(), Error> {
        let (dir, ns_entries) = create_fs_with_mock_logsink()?;

        let ((), actual) = try_join!(
            write_to_syslog_or_panic(ns_entries, b"Hello \xF0\x90\x80World!"),
            read_message_from_syslog(dir)
        )?;

        assert_eq!(actual, Some("Hello �World!".to_owned()));
        Ok(())
    }

    #[fuchsia::test]
    async fn drain_lines_splits_into_max_size_chunks() -> Result<(), Error> {
        let (tx, rx) = zx::Socket::create_stream();
        let rx = fasync::Socket::from_socket(rx);
        let (mut sender, recv) = create_mock_logger();
        let msg = get_random_string(MAX_MESSAGE_SIZE * 4);

        let () = take_and_write_to_socket(tx, &msg)?;
        let (actual, ()) =
            try_join!(recv.collect().map(Result::<Vec<String>, Error>::Ok), async move {
                drain_lines(rx, &mut sender).await.map_err(Into::into)
            })?;

        assert_eq!(
            actual,
            msg.as_bytes()
                .chunks(MAX_MESSAGE_SIZE)
                .map(|bytes| std::str::from_utf8(bytes).expect("Bytes are not utf8.").to_owned())
                .collect::<Vec<String>>()
        );

        Ok(())
    }

    #[fuchsia::test]
    async fn drain_lines_splits_at_newline() -> Result<(), Error> {
        let (tx, rx) = zx::Socket::create_stream();
        let rx = fasync::Socket::from_socket(rx);
        let (mut sender, recv) = create_mock_logger();
        let msg = std::iter::repeat_with(|| {
            Alphanumeric.sample_string(&mut thread_rng(), MAX_MESSAGE_SIZE - 1)
        })
        .take(3)
        .collect::<Vec<_>>()
        .join("\n");

        let () = take_and_write_to_socket(tx, &msg)?;
        let (actual, ()) =
            try_join!(recv.collect().map(Result::<Vec<String>, Error>::Ok), async move {
                drain_lines(rx, &mut sender).await.map_err(Into::into)
            })?;

        assert_eq!(actual, msg.split("\n").map(str::to_owned).collect::<Vec<String>>());
        Ok(())
    }

    #[fuchsia::test]
    async fn drain_lines_writes_when_message_is_received() -> Result<(), Error> {
        let (tx, rx) = zx::Socket::create_stream();
        let rx = fasync::Socket::from_socket(rx);
        let (mut sender, mut recv) = create_mock_logger();
        let messages: Vec<String> = vec!["Hello!\n".to_owned(), "World!\n".to_owned()];

        let ((), ()) = try_join!(
            async move { drain_lines(rx, &mut sender).await.map_err(Error::from) },
            async move {
                for mut message in messages.into_iter() {
                    let () = write_to_socket(&tx, &message)?;
                    let logged_messaged =
                        recv.next().await.context("Receiver channel closed. Got no message.")?;
                    // Logged message should strip '\n' so we need to do the same before assertion.
                    message.pop();
                    assert_eq!(logged_messaged, message);
                }

                Ok(())
            }
        )?;

        Ok(())
    }

    #[fuchsia::test]
    async fn drain_lines_waits_for_entire_lines() -> Result<(), Error> {
        let (tx, rx) = zx::Socket::create_stream();
        let rx = fasync::Socket::from_socket(rx);
        let (mut sender, mut recv) = create_mock_logger();

        let ((), ()) = try_join!(
            async move { drain_lines(rx, &mut sender).await.map_err(Error::from) },
            async move {
                let () = write_to_socket(&tx, "Hello\nWorld")?;
                let logged_messaged =
                    recv.next().await.context("Receiver channel closed. Got no message.")?;
                assert_eq!(logged_messaged, "Hello");
                let () = write_to_socket(&tx, "Hello\nAgain")?;
                std::mem::drop(tx);
                let logged_messaged =
                    recv.next().await.context("Receiver channel closed. Got no message.")?;
                assert_eq!(logged_messaged, "WorldHello");
                let logged_messaged =
                    recv.next().await.context("Receiver channel closed. Got no message.")?;
                assert_eq!(logged_messaged, "Again");
                Ok(())
            }
        )?;

        Ok(())
    }

    #[fuchsia::test]
    async fn drain_lines_collapses_repeated_newlines() -> Result<(), Error> {
        let (tx, rx) = zx::Socket::create_stream();
        let rx = fasync::Socket::from_socket(rx);
        let (mut sender, mut recv) = create_mock_logger();

        let drainer = Task::spawn(async move { drain_lines(rx, &mut sender).await });

        write_to_socket(&tx, "Hello\n\nWorld\n")?;
        assert_eq!(recv.next().await.unwrap(), "Hello");
        assert_eq!(recv.next().await.unwrap(), "World");

        drop(tx);
        drainer.await?;
        assert_eq!(recv.next().await, None);

        Ok(())
    }

    async fn write_to_syslog_or_panic(
        ns_entries: Vec<fcrunner::ComponentNamespaceEntry>,
        message: &[u8],
    ) -> Result<(), Error> {
        let ns = Namespace::try_from(ns_entries).context("Failed to create Namespace")?;
        let logger = create_namespace_logger(&ns).context("Failed to create ScopedLogger")?;
        let mut writer = SyslogWriter::new(Arc::new(logger), OutputLevel::Info);
        writer.write(message).await;

        Ok(())
    }

    /// Retrieve message logged to socket. The wire format is expected to
    /// match with the LogSink protocol format.
    pub fn get_message_logged_to_socket(socket: zx::Socket) -> Option<String> {
        let mut buffer: [u8; 1024] = [0; 1024];
        match socket.read(&mut buffer) {
            Ok(read_len) => {
                let msg = diagnostics_message::from_structured(
                    MonikerWithUrl {
                        url: "fuchsia-pkg://fuchsia.com/test-pkg#meta/test-component.cm".into(),
                        moniker: "test-pkg/test-component".try_into().unwrap(),
                    },
                    &buffer[..read_len],
                )
                .expect("must be able to decode a valid message from buffer");

                msg.msg().map(String::from)
            }
            Err(_) => None,
        }
    }

    async fn read_message_from_syslog(
        dir: MockServiceFs<'static>,
    ) -> Result<Option<String>, Error> {
        let message_logged = Arc::new(Mutex::new(Option::<String>::None));
        dir.for_each_concurrent(None, |request: MockServiceRequest| match request {
            MockServiceRequest::LogSink(mut r) => {
                let message_logged_copy = Arc::clone(&message_logged);
                async move {
                    match r.next().await.expect("stream error").expect("fidl error") {
                        LogSinkRequest::Connect { .. } => {
                            panic!("Unexpected call to `Connect`");
                        }
                        LogSinkRequest::ConnectStructured { socket, .. } => {
                            *message_logged_copy.lock().unwrap() =
                                get_message_logged_to_socket(socket);
                        }
                        LogSinkRequest::WaitForInterestChange { .. } => {
                            // we expect this request to come but asserting on it is flakey
                        }
                        LogSinkRequest::_UnknownMethod { .. } => {
                            panic!("Unexpected unknown method")
                        }
                    }
                }
            }
        })
        .await;

        let message_logged =
            message_logged.lock().map_err(|_| anyhow!("Failed to lock mutex"))?.clone();
        Ok(message_logged)
    }

    fn take_and_write_to_socket(socket: zx::Socket, message: &str) -> Result<(), Error> {
        write_to_socket(&socket, &message)
    }

    fn write_to_socket(socket: &zx::Socket, message: &str) -> Result<(), Error> {
        let bytes_written =
            socket.write(message.as_bytes()).context("Failed to write to socket")?;
        match bytes_written == message.len() {
            true => Ok(()),
            false => Err(format_err!("Bytes written to socket doesn't match len of message. Message len = {}. Bytes written = {}", message.len(), bytes_written)),
        }
    }

    fn create_mock_logger() -> (mpsc::Sender<String>, mpsc::Receiver<String>) {
        mpsc::channel::<String>(20)
    }

    fn get_random_string(size: usize) -> String {
        Alphanumeric.sample_string(&mut thread_rng(), size)
    }
}
