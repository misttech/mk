// Copyright 2024 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! A basic [`Transport`] implementation based on MPSC channels.

use core::fmt;
use core::future::Future;
use core::mem::take;
use core::pin::Pin;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::task::{Context, Poll};
use std::sync::{mpsc, Arc};

use fidl_next_codec::decoder::InternalHandleDecoder;
use fidl_next_codec::{Chunk, DecodeError, Decoder, CHUNK_SIZE};
use futures::task::AtomicWaker;

use crate::Transport;

struct SharedEnd {
    sender_count: AtomicUsize,
    send_waker: AtomicWaker,
}

struct Shared {
    is_closed: AtomicBool,
    ends: [SharedEnd; 2],
}

impl Shared {
    fn close(&self) {
        let was_closed = self.is_closed.swap(true, Ordering::Relaxed);
        if !was_closed {
            for end in &self.ends {
                end.send_waker.wake();
            }
        }
    }
}

/// A paired mpsc transport.
pub struct Mpsc {
    sender: Sender,
    receiver: mpsc::Receiver<Vec<Chunk>>,
}

impl Mpsc {
    /// Creates two mpscs which can communicate with each other.
    pub fn new() -> (Self, Self) {
        let shared = Arc::new(Shared {
            is_closed: AtomicBool::new(false),
            ends: [
                SharedEnd { sender_count: AtomicUsize::new(1), send_waker: AtomicWaker::new() },
                SharedEnd { sender_count: AtomicUsize::new(1), send_waker: AtomicWaker::new() },
            ],
        });
        let (a_send, a_recv) = mpsc::channel();
        let (b_send, b_recv) = mpsc::channel();
        (
            Mpsc {
                sender: Sender { shared: shared.clone(), end: 0, sender: a_send },
                receiver: b_recv,
            },
            Mpsc { sender: Sender { shared, end: 1, sender: b_send }, receiver: a_recv },
        )
    }
}

/// The error type for paired mpsc transports.
#[derive(Debug)]
pub enum Error {
    /// The mpsc was closed.
    Closed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => write!(f, "the mpsc was closed"),
        }
    }
}

impl core::error::Error for Error {}

/// The send end of a paired mpsc transport.
pub struct Sender {
    shared: Arc<Shared>,
    end: usize,
    sender: mpsc::Sender<Vec<Chunk>>,
}

impl Clone for Sender {
    fn clone(&self) -> Self {
        self.shared.ends[self.end].sender_count.fetch_add(1, Ordering::Relaxed);
        Self { shared: self.shared.clone(), end: self.end, sender: self.sender.clone() }
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        let senders = self.shared.ends[self.end].sender_count.fetch_sub(1, Ordering::Relaxed);
        if senders == 1 {
            self.shared.close();
        }
    }
}

/// The send future for a paired mpsc transport.
pub struct SendFuture<'s> {
    sender: &'s Sender,
    buffer: Vec<Chunk>,
}

impl Future for SendFuture<'_> {
    type Output = Result<(), Error>;

    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        if self.sender.shared.is_closed.load(Ordering::Relaxed) {
            return Poll::Ready(Err(Error::Closed));
        }

        let chunks = take(&mut self.buffer);
        match self.sender.sender.send(chunks) {
            Ok(()) => {
                self.sender.shared.ends[self.sender.end].send_waker.wake();
                Poll::Ready(Ok(()))
            }
            Err(_) => Poll::Ready(Err(Error::Closed)),
        }
    }
}

/// The receive end of a paired mpsc transport.
pub struct Receiver {
    shared: Arc<Shared>,
    end: usize,
    receiver: mpsc::Receiver<Vec<Chunk>>,
}

/// The receive future for a paired mpsc transport.
pub struct RecvFuture<'r> {
    receiver: &'r mut Receiver,
}

impl Future for RecvFuture<'_> {
    type Output = Result<Option<RecvBuffer>, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.receiver.shared.is_closed.load(Ordering::Relaxed) {
            return Poll::Ready(Ok(None));
        }

        self.receiver.shared.ends[1 - self.receiver.end].send_waker.register(cx.waker());
        match self.receiver.receiver.try_recv() {
            Ok(chunks) => Poll::Ready(Ok(Some(RecvBuffer { chunks, chunks_taken: 0 }))),
            Err(mpsc::TryRecvError::Empty) => Poll::Pending,
            Err(mpsc::TryRecvError::Disconnected) => Poll::Ready(Ok(None)),
        }
    }
}

/// A received message buffer.
pub struct RecvBuffer {
    chunks: Vec<Chunk>,
    chunks_taken: usize,
}

impl InternalHandleDecoder for RecvBuffer {
    fn __internal_take_handles(&mut self, _: usize) -> Result<(), DecodeError> {
        Err(DecodeError::InsufficientHandles)
    }

    fn __internal_handles_remaining(&self) -> usize {
        0
    }
}

unsafe impl Decoder for RecvBuffer {
    fn take_chunks_raw(&mut self, count: usize) -> Result<NonNull<Chunk>, DecodeError> {
        if count > self.chunks.len() - self.chunks_taken {
            return Err(DecodeError::InsufficientData);
        }

        let chunks = unsafe { self.chunks.as_mut_ptr().add(self.chunks_taken) };
        self.chunks_taken += count;

        unsafe { Ok(NonNull::new_unchecked(chunks)) }
    }

    fn finish(&mut self) -> Result<(), DecodeError> {
        if self.chunks_taken != self.chunks.len() {
            return Err(DecodeError::ExtraBytes {
                num_extra: (self.chunks.len() - self.chunks_taken) * CHUNK_SIZE,
            });
        }

        Ok(())
    }
}

impl Transport for Mpsc {
    type Error = Error;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        let receiver = Receiver {
            shared: self.sender.shared.clone(),
            end: self.sender.end,
            receiver: self.receiver,
        };
        (self.sender, receiver)
    }

    type Sender = Sender;
    type SendBuffer = Vec<Chunk>;
    type SendFuture<'s> = SendFuture<'s>;

    fn acquire(_: &Self::Sender) -> Self::SendBuffer {
        Vec::new()
    }

    fn send(sender: &Self::Sender, buffer: Self::SendBuffer) -> Self::SendFuture<'_> {
        SendFuture { sender, buffer }
    }

    fn close(sender: &Self::Sender) {
        sender.shared.close();
    }

    type Receiver = Receiver;
    type RecvFuture<'r> = RecvFuture<'r>;
    type RecvBuffer = RecvBuffer;

    fn recv(receiver: &mut Self::Receiver) -> Self::RecvFuture<'_> {
        RecvFuture { receiver }
    }
}

#[cfg(test)]
mod tests {
    use fuchsia_async as fasync;

    use super::Mpsc;
    use crate::testing::*;

    #[fasync::run_singlethreaded(test)]
    async fn close_on_drop() {
        let (client_end, server_end) = Mpsc::new();
        test_close_on_drop(client_end, server_end).await;
    }

    #[fasync::run_singlethreaded(test)]
    async fn send_receive() {
        let (client_end, server_end) = Mpsc::new();
        test_one_way(client_end, server_end).await;
    }

    #[fasync::run_singlethreaded(test)]
    async fn two_way() {
        let (client_end, server_end) = Mpsc::new();
        test_two_way(client_end, server_end).await;
    }

    #[fasync::run_singlethreaded(test)]
    async fn multiple_two_way() {
        let (client_end, server_end) = Mpsc::new();
        test_multiple_two_way(client_end, server_end).await;
    }

    #[fasync::run_singlethreaded(test)]
    async fn event() {
        let (client_end, server_end) = Mpsc::new();
        test_event(client_end, server_end).await;
    }
}
