//! Callback-based asynchronous operation monitoring
//!
//! This module provides a way to monitor asynchronous operations through
//! asynchronous callbacks. It is more flexible than polling and blocking (which
//! could technically be implemented on top of it), and can achieve higher
//! performance, but at the cost of somewhat higher code complexity.

use client::IAsyncOpClient;
use executor::{CallbackExecutor, AnyCallbackChannel};
use server::{self, AsyncOpServerConfig};
use status::{AsyncOpStatus, AsyncOpStatusDetails};
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};


/// Asynchronous operation object
pub struct AsyncOp<Details: AsyncOpStatusDetails + 'static,
                   Channel: AnyCallbackChannel>
{
    /// Server interface used to submit status updates
    server: AsyncOpServer<Details, Channel>,

    /// Client interface used to monitor the operation status
    client: AsyncOpClient,
}
//
impl<Details: AsyncOpStatusDetails,
     Channel: AnyCallbackChannel>
AsyncOp<Details, Channel> {
    // NOTE: In this module, the new operator cannot be a struct method, because
    //       the "Channel" type parameter depends on constructor parameters

    /// Split the asynchronous operation object into client and server
    /// objects which can be respectively sent to client and server threads
    pub fn split(self) -> (AsyncOpServer<Details, Channel>, AsyncOpClient) {
        (self.server, self.client)
    }
}


/// EXTERNAL constructor of asynchronous operations
pub fn new_async_op<Details: AsyncOpStatusDetails + 'static,
                    F: Fn(AsyncOpStatus<Details>) + 'static,
                    Executor: CallbackExecutor>(
    callback: F,
    executor: &mut Executor,
    initial_status: AsyncOpStatus<Details>
) -> AsyncOp<Details, Executor::Channel> {
    // Setup a callback channel on the active executor...
    let callback_channel = executor.setup_callback(callback);

    // ...and a shared cancellation flag...
    let cancel_flag = Arc::new(AtomicBool::new(false));

    // ...then build the asynchronous operation client and serer
    AsyncOp {
        server: AsyncOpServer::new(
            CallbackServerConfig {
                channel: callback_channel,
                cancelled: cancel_flag.clone(),
                details: PhantomData,
            },
            &initial_status
        ),
        client: AsyncOpClient {
            cancelled: cancel_flag,
        },
    }
}


/// Server interface, used to send operation status updates to the client
pub type AsyncOpServer<Details: AsyncOpStatusDetails + 'static,
                       Channel: AnyCallbackChannel> =
    server::AsyncOpServer<CallbackServerConfig<Details, Channel>>;


/// Server configuration for callback-based operation monitoring
pub struct CallbackServerConfig<Details: AsyncOpStatusDetails + 'static,
                                CallbackChannel: AnyCallbackChannel> {
    /// The following callback channel will receive our status updates
    channel: CallbackChannel,

    /// In addition, the client & server also share a cancellation flag
    cancelled: Arc<AtomicBool>,

    /// We need to remember our status details because AnyCallbackChannel won't
    /// be able to do it for us
    details: PhantomData<Details>,
}
//
impl<Details: AsyncOpStatusDetails + 'static,
     CallbackChannel: AnyCallbackChannel>
AsyncOpServerConfig for CallbackServerConfig<Details, CallbackChannel>
{
    /// Implementation details of the asynchronous operation status
    type StatusDetails = Details;

    /// Method used to send a status update to the client
    fn update(&mut self, status: AsyncOpStatus<Details>) {
        self.channel.notify(status);
    }

    /// Method used to query whether the client has cancelled the operation
    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}


/// Client interface, used to cancel the asynchronous operation
pub struct AsyncOpClient {
    /// In callback-based synchronization, all the client can do is cancel
    cancelled: Arc<AtomicBool>,
}
//
impl IAsyncOpClient for AsyncOpClient {
    fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}



/// Unit tests
#[cfg(test)]
mod tests {
    use executor::inline::InlineCallbackExecutor;
    use multithread::callback::*;
    use status::{self, StandardAsyncOpStatus};
    use std::cell::Cell;
    use std::rc::Rc;

    /// Check the initial operation state
    #[test]
    #[allow(unused_variables)]
    fn initial_state() {
        // This callback will set a boolean flag if called
        let called = Rc::new(Cell::new(false));
        let c_called = called.clone();
        let callback = move | s: StandardAsyncOpStatus | c_called.set(true);

        // Check that the callback does not get called during server creation
        let mut executor = InlineCallbackExecutor::new();
        let async_op = new_async_op(callback, &mut executor, status::PENDING);
        assert!(!called.get());
    }

    /// Check that the callback is called on status updates
    #[test]
    fn update() {
        // This callback will increment a counter if called
        let counter = Rc::new(Cell::new(0));
        let c_counter = counter.clone();
        let callback = move | s: StandardAsyncOpStatus | {
            assert_eq!(s, status::DONE);
            c_counter.set(c_counter.get() + 1);
        };

        // Check that the callback gets called exactly once on status updates
        let mut executor = InlineCallbackExecutor::new();
        let async_op = new_async_op(callback, &mut executor, status::PENDING);
        let (mut server, _) = async_op.split();
        server.update(status::DONE);
        assert_eq!(counter.get(), 1);
    }

    /// Check that cancellation works as expected
    #[test]
    #[allow(unused_variables)]
    fn cancelation() {
        // This callback will set a boolean flag if called
        let called = Rc::new(Cell::new(false));
        let c_called = called.clone();
        let callback = move | s: StandardAsyncOpStatus | c_called.set(true);

        // Create a test harness
        let mut executor = InlineCallbackExecutor::new();
        let async_op = new_async_op(callback, &mut executor, status::PENDING);
        let (server, mut client) = async_op.split();

        // Check that cancellation works as expected
        client.cancel();
        assert!(server.cancelled());
        assert!(!called.get());
    }
}


// TODO: Add benchmarks
