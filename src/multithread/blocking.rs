//! Blocking-based asynchronous operation monitoring
//!
//! This module provides a way to monitor asynchronous operations by blocking
//! until their status changes. This synchronization mechanism is easy to use
//! and reason about, but should be used with care as the unpredictable
//! application delays that it introduces can be harmful to performance.

use server::{GenericAsyncOpServer, AsyncOpServerConfig};
use status::{self, AsyncOpStatus, AsyncOpStatusDetails};
use std::sync::{Arc, Mutex, Condvar};

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

        // Start by building the shared state...
        let shared_state = Arc::new(
            SharedState {
                status_lock: Mutex::new(
                    StatusWithReadBit {
                        status: initial_status,
                        read: false,
                    }
                ),
                update_cv: Condvar::new(),
            }
        );

        // ...then build the client and server
        AsyncOp {
            server: GenericAsyncOpServer::new(
                BlockingServerConfig { shared: shared_state.clone() },
                &initial_status_copy
            ),
            client: AsyncOpClient { shared: shared_state },
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
    GenericAsyncOpServer<BlockingServerConfig<Details>>;


/// Server configuration for blocking operation monitoring
pub struct BlockingServerConfig<Details: AsyncOpStatusDetails> {
    /// Reference-counted shared state
    shared: Arc<SharedState<Details>>,
}
//
impl<Details: AsyncOpStatusDetails> AsyncOpServerConfig
    for BlockingServerConfig<Details>
{
    /// Implementation details of the asynchronous operation status
    type StatusDetails = Details;

    /// Method used to send a status update to the client
    fn update(&mut self, status: AsyncOpStatus<Details>) {
        // Update the value of the asynchronous operation status
        *self.shared.status_lock
                    .lock()
                    .unwrap() = StatusWithReadBit { status: status,
                                                    read: false };

        // Notify the reader that an update has occured
        self.shared.update_cv.notify_all();
    }
}


/// Client interface, used to synchronize with the operation status
pub struct AsyncOpClient<Details: AsyncOpStatusDetails> {
    /// Reference-counted shared state
    shared: Arc<SharedState<Details>>,
}
//
impl<Details: AsyncOpStatusDetails> AsyncOpClient<Details> {
    /// Clone the current asynchronous operation status
    pub fn status(&self) -> AsyncOpStatus<Details> {
        (*self.shared.status_lock.lock().unwrap()).status.clone()
    }

    /// Wait for either a status update or a final operation status
    pub fn wait(&self) -> AsyncOpStatus<Details> {
        // Access the current operation status
        let mut status_lock = self.shared.status_lock.lock().unwrap();

        // Only wait if the current status was read and can still change
        if status_lock.read && !status::is_final(&status_lock.status) {
            let wait_result = self.shared.update_cv.wait(status_lock);
            status_lock = wait_result.unwrap();
        }

        // Mark the current operation status as read
        status_lock.read = true;

        // Return the final operation status
        status_lock.status.clone()
    }
}


/// State shared between the client and the server
struct SharedState<Details: AsyncOpStatusDetails> {
    /// Current asynchronous operation status (mutex-protected)
    status_lock: Mutex<StatusWithReadBit<Details>>,

    /// Condition variable used to notify clients about status updates
    update_cv: Condvar,
}
//
struct StatusWithReadBit<Details: AsyncOpStatusDetails> {
    /// Current asynchronous operation status
    status: AsyncOpStatus<Details>,

    /// Whether this version of the status was read by the client
    read: bool,
}


// TODO: Add tests and benchmarks
