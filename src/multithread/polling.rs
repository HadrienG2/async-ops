//! Polling-based asynchronous operation monitoring
//!
//! This module provides a way to monitor asynchronous operations through
//! polling. It provides maximal performance in scenarios where a client does
//! not need to synchronize with asynchronous operation status updates, but only
//! to periodically check the status, as is the case for example when updating
//! progress bars and status graphs in user interfaces.

use client::IAsyncOpClient;
use server::{self, AsyncOpServerConfig};
use status::{AsyncOpStatus, AsyncOpStatusDetails};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use triple_buffer::{TripleBuffer, TripleBufferInput, TripleBufferOutput};


/// Asynchronous operation object
pub struct AsyncOp<Details: AsyncOpStatusDetails> {
    /// Server interface used to submit status updates
    server: AsyncOpServer<Details>,

    /// Client interface used to monitor the operation status
    client: AsyncOpClient<Details>,
}
//
impl<Details: AsyncOpStatusDetails> AsyncOp<Details> {
    /// Create a new asynchronous operation object with some initial status
    pub fn new(initial_status: AsyncOpStatus<Details>) -> Self {
        // Keep a copy of the initial operation status
        let initial_status_copy = initial_status.clone();

        // Setup triple buffer-based client/server communication...
        let buffer = TripleBuffer::new(initial_status);
        let (buf_input, buf_output) = buffer.split();

        // ...and a shared cancellation flag...
        let cancel_flag = Arc::new(AtomicBool::new(false));

        // ...then build the client and server
        AsyncOp {
            server: AsyncOpServer::new(
                PollingServerConfig {
                    buf_input: buf_input,
                    cancelled: cancel_flag.clone(),
                },
                &initial_status_copy
            ),
            client: AsyncOpClient {
                buf_output: buf_output,
                cancelled: cancel_flag,
            },
        }
    }

    /// Split the asynchronous operation object into client and server
    /// objects which can be respectively sent to client and server threads
    pub fn split(self) -> (AsyncOpServer<Details>, AsyncOpClient<Details>) {
        (self.server, self.client)
    }
}


/// Server interface, used to send operation status updates to the client
pub type AsyncOpServer<Details: AsyncOpStatusDetails> =
    server::AsyncOpServer<PollingServerConfig<Details>>;


/// Server configuration for polling-based operation monitoring
pub struct PollingServerConfig<Details: AsyncOpStatusDetails> {
    /// New operation statuses will be sent through this triple buffer
    buf_input: TripleBufferInput<AsyncOpStatus<Details>>,

    /// In addition, the client & server also share a cancellation flag
    cancelled: Arc<AtomicBool>,
}
//
impl<Details: AsyncOpStatusDetails> AsyncOpServerConfig
    for PollingServerConfig<Details>
{
    /// Implementation details of the asynchronous operation status
    type StatusDetails = Details;

    /// Method used to send a status update to the client
    fn update(&mut self, status: AsyncOpStatus<Details>) {
        self.buf_input.write(status);
    }

    /// Method used to query whether the client has cancelled the operation
    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}


/// Client interface, used to synchronize with the operation status
pub struct AsyncOpClient<Details: AsyncOpStatusDetails> {
    /// Current operation status will be read through this triple buffer
    buf_output: TripleBufferOutput<AsyncOpStatus<Details>>,

    /// In addition, the client & server also share a cancellation flag
    cancelled: Arc<AtomicBool>,
}
//
impl<Details: AsyncOpStatusDetails> AsyncOpClient<Details> {
    /// Access the current asynchronous operation status
    pub fn status(&mut self) -> &AsyncOpStatus<Details> {
        self.buf_output.read()
    }
}
//
impl<Details: AsyncOpStatusDetails> IAsyncOpClient for AsyncOpClient<Details> {
    /// Request the cancellation of the active asynchronous operation
    fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}


/// Unit tests
#[cfg(test)]
mod tests {
    use multithread::polling::*;
    use status;

    /// Check the initial state of asynchronous operations
    #[test]
    #[allow(unused_variables)]
    fn initial_state() {
        // Does the client give out the right status initially?
        let mut async_op = AsyncOp::new(status::PENDING);
        assert_eq!(*async_op.client.status(), status::PENDING);

        // Does it still give it out after splitting the client and the server?
        let (server, mut client) = async_op.split();
        assert_eq!(*client.status(), status::PENDING);

        // Is the cancellation flag initially unset?
        assert!(!server.cancelled());
    }

    /// Check that status changes propagate correctly from client to server
    #[test]
    fn status_propagation() {
        let async_op = AsyncOp::new(status::PENDING);
        let (mut server, mut client) = async_op.split();
        server.update(status::DONE);
        assert_eq!(*client.status(), status::DONE);
    }

    /// Check that cancellation works as expected
    #[test]
    fn cancelation() {
        // Create an asynchronous operation
        let async_op = AsyncOp::new(status::PENDING);
        let (server, mut client) = async_op.split();

        // Make sure that cancelling it works as expected
        client.cancel();
        assert!(server.cancelled());
    }
}


// TODO: Add benchmarks
