//! Polling-based asynchronous operation monitoring
//!
//! This module provides a way to monitor asynchronous operations through
//! polling. It provides maximal performance in scenarios where a client does
//! not need to synchronize with an asynchronous operation, but only to
//! periodically check its status, as is the case for example when updating
//! progress bars and status graphs in user interfaces.

use server::{GenericAsyncOpServer, AsyncOpServerConfig};
use status::{AsyncOpStatus, AsyncOpStatusDetails};
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

        // ...then build the client and server
        AsyncOp {
            server: GenericAsyncOpServer::new(
                PollingServerConfig { buf_input: buf_input },
                &initial_status_copy
            ),
            client: AsyncOpClient { buf_output: buf_output },
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
    GenericAsyncOpServer<PollingServerConfig<Details>>;


/// Server configuration for polling-based operation monitoring
pub struct PollingServerConfig<Details: AsyncOpStatusDetails> {
    /// New operation statuses will be sent through this triple buffer
    buf_input: TripleBufferInput<AsyncOpStatus<Details>>,
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
}


/// Client interface, used to synchronize with the operation status
pub struct AsyncOpClient<Details: AsyncOpStatusDetails> {
    /// Current operation status will be read through this triple buffer
    buf_output: TripleBufferOutput<AsyncOpStatus<Details>>,
}
//
impl<Details: AsyncOpStatusDetails> AsyncOpClient<Details> {
    /// Access the current asynchronous operation status
    pub fn status(&mut self) -> &AsyncOpStatus<Details> {
        self.buf_output.read()
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
    }

    /// Check that status changes propagate correctly from client to server
    #[test]
    fn status_propagation() {
        let async_op = AsyncOp::new(status::PENDING);
        let (mut server, mut client) = async_op.split();
        server.update(status::DONE);
        assert_eq!(*client.status(), status::DONE);
    }
}


// TODO: Add benchmarks
