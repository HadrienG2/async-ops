//! Callback-based asynchronous operation monitoring
//!
//! This module provides a way to monitor asynchronous operations through
//! asynchronous callbacks. It is more flexible than polling and blocking (which
//! could technically be implemented on top of it), and can achieve higher
//! performance, but at the cost of somewhat higher code complexity.

use server::{GenericAsyncOpServer, AsyncOpServerConfig};
use status::{AsyncOpStatus, AsyncOpStatusDetails};


// TODO: Ajouter des exécuteurs


/// With callbacks, there is only an asynchronous operation server to create
fn new_callback_server<Details: AsyncOpStatusDetails,
                       F: Fn(AsyncOpStatus<Details>) + 'static>(
    callback: F,
    initial_status: AsyncOpStatus<Details>
) -> AsyncOpServer<Details> {
    GenericAsyncOpServer::new(
        CallbackServerConfig { callback: Box::new(callback) },
        &initial_status
    )
}


/// Server interface, used to send operation status updates to the client
pub type AsyncOpServer<Details: AsyncOpStatusDetails> =
    GenericAsyncOpServer<CallbackServerConfig<Details>>;


/// Server configuration for callback-based operation monitoring
pub struct CallbackServerConfig<Details: AsyncOpStatusDetails> {
    /// The following callback will be invoked on every status change
    callback: Box<Fn(AsyncOpStatus<Details>)>,
}
//
impl<Details: AsyncOpStatusDetails> AsyncOpServerConfig
    for CallbackServerConfig<Details>
{
    /// Implementation details of the asynchronous operation status
    type StatusDetails = Details;

    /// Method used to send a status update to the client
    fn update(&mut self, status: AsyncOpStatus<Details>) {
        (self.callback)(status);
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
        let server = new_callback_server(callback, status::PENDING);
        assert!(!called.get());
    }

    /// Check that the callback is called on status updates
    #[test]
    #[allow(unused_variables)]
    fn update() {
        // This callback will increment a counter if called
        let counter = Rc::new(Cell::new(0));
        let c_counter = counter.clone();
        let callback = move | s: StandardAsyncOpStatus | {
            c_counter.set(c_counter.get() + 1);
        };

        // Check that the callback gets called exactly once on status updates
        let mut server = new_callback_server(callback, status::PENDING);
        server.update(status::DONE);
        assert_eq!(counter.get(), 1);
    }
}


// TODO: Add benchmarks
