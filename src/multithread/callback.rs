//! Callback-based asynchronous operation monitoring
//!
//! This module provides a way to monitor asynchronous operations through
//! asynchronous callbacks. It is more flexible than polling and blocking (which
//! could technically be implemented on top of it), and can achieve higher
//! performance, but at the cost of somewhat higher code complexity.

use executor::{CallbackExecutor, AnyCallbackChannel};
use executor::inline::InlineCallbackExecutor;
use server::{self, AsyncOpServerConfig};
use status::{AsyncOpStatus, AsyncOpStatusDetails};
use std::marker::PhantomData;


/// With callbacks, there is only an asynchronous operation server to create
fn new_callback_server<Details: AsyncOpStatusDetails + 'static,
                       F: Fn(AsyncOpStatus<Details>) + 'static,
                       Executor: CallbackExecutor>(
    callback: F,
    executor: &mut Executor,
    initial_status: AsyncOpStatus<Details>
) -> AsyncOpServer<Details, Executor::Channel> {
    // Setup a callback channel on the active executor
    let callback_channel = executor.setup_callback(callback);

    // Setup the asynchronous operation server with that channel
    AsyncOpServer::new(
        CallbackServerConfig {
            channel: callback_channel,
            details: PhantomData
        },
        &initial_status
    )
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
}


/// Unit tests
#[cfg(test)]
mod tests {
    use multithread::callback::*;
    use status::{self, StandardAsyncOpStatus};
    use std::cell::Cell;
    use std::rc::Rc;

    /// Check the callback does not get called on operation creation
    #[test]
    #[allow(unused_variables)]
    fn initial_state() {
        // This callback will set a boolean flag if called
        let called = Rc::new(Cell::new(false));
        let c_called = called.clone();
        let callback = move | s: StandardAsyncOpStatus | c_called.set(true);

        // Check that the callback does not get called on server creation
        let mut executor = InlineCallbackExecutor::new();
        let server = new_callback_server(callback,
                                         &mut executor,
                                         status::PENDING);
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
        let mut server = new_callback_server(callback,
                                             &mut executor,
                                             status::PENDING);
        server.update(status::DONE);
        assert_eq!(counter.get(), 1);
    }
}


// TODO: Add benchmarks
