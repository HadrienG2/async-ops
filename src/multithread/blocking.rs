//! Blocking-based asynchronous operation monitoring
//!
//! This module provides a way to monitor asynchronous operations by blocking
//! until their status changes. This synchronization mechanism is easy to use
//! and reason about, but should be used with care as the unpredictable
//! application delays that it introduces can be harmful to performance.

use client::IAsyncOpClient;
use server::{self, AsyncOpServerConfig};
use status::{self, AsyncOpStatus, AsyncOpStatusDetails};
use std::sync::{Arc, Mutex, Condvar};
use std::sync::atomic::{AtomicBool, Ordering};

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
                cancelled: AtomicBool::new(false),
            }
        );

        // ...then build the client and server
        AsyncOp {
            server: AsyncOpServer::new(
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
    server::AsyncOpServer<BlockingServerConfig<Details>>;


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

    /// Method used to query whether the client has cancelled the operation
    fn cancelled(&self) -> bool {
        self.shared.cancelled.load(Ordering::Acquire)
    }
}


/// Client interface, used to synchronize with the operation status
pub struct AsyncOpClient<Details: AsyncOpStatusDetails> {
    /// Reference-counted shared state
    shared: Arc<SharedState<Details>>,
}
//
impl<Details: AsyncOpStatusDetails> AsyncOpClient<Details> {
    /// Access the current operation status and mark it as read
    pub fn status(&mut self) -> AsyncOpStatus<Details> {
        // Access the current operation status
        let mut status_lock = self.shared.status_lock.lock().unwrap();

        // Mark it as read
        status_lock.read = true;

        // Return a copy of it
        status_lock.status.clone()
    }

    /// Wait for either a status update or a final operation status
    pub fn wait(&mut self) -> AsyncOpStatus<Details> {
        // Access the current operation status
        let mut status_lock = self.shared.status_lock.lock().unwrap();

        // Only wait if the current status was read and can still change
        while status_lock.read && !status::is_final(&status_lock.status) {
            let wait_result = self.shared.update_cv.wait(status_lock);
            status_lock = wait_result.unwrap();
        }

        // Mark the current operation status as read
        status_lock.read = true;

        // Return a copy of the final operation status
        status_lock.status.clone()
    }
}
//
impl<Details: AsyncOpStatusDetails> IAsyncOpClient for AsyncOpClient<Details> {
    /// Request the cancellation of the active asynchronous operation
    fn cancel(&mut self) {
        self.shared.cancelled.store(true, Ordering::Release);
    }
}


/// State shared between the client and the server
struct SharedState<Details: AsyncOpStatusDetails> {
    /// Current asynchronous operation status (mutex-protected)
    status_lock: Mutex<StatusWithReadBit<Details>>,

    /// Condition variable used to notify clients about status updates
    update_cv: Condvar,

    /// Atomic boolean used by the client to request cancellation
    cancelled: AtomicBool,
}
//
struct StatusWithReadBit<Details: AsyncOpStatusDetails> {
    /// Current asynchronous operation status
    status: AsyncOpStatus<Details>,

    /// Whether this version of the status was read by the client
    read: bool,
}


/// Unit tests
#[cfg(test)]
mod tests {
    use multithread::blocking::*;
    use status;
    use std::sync::{Arc, Condvar};
    use std::thread;
    use std::time::Duration;

    /// Check the initial state of asynchronous operations
    #[test]
    fn initial_state() {
        // Access the initial value of the operation's shared state
        let async_op = AsyncOp::new(status::PENDING);
        let shared_state = async_op.client.shared;

        // Is the initial operation status correct and unread?
        let status_lock = shared_state.status_lock.lock().unwrap();
        assert_eq!(status_lock.status, status::PENDING);
        assert_eq!(status_lock.read, false);

        // Is it mistakenly cancelled?
        let cancelled = shared_state.cancelled.load(Ordering::Relaxed);
        assert!(!cancelled);
    }

    /// Check that reading the operation status marks it as read
    #[test]
    #[allow(unused_variables)]
    fn mark_read() {
        // Check that reading the initial operation status works
        let async_op = AsyncOp::new(status::RUNNING);
        let (server, mut client) = async_op.split();
        assert_eq!(client.status(), status::RUNNING);

        // Check that it marks the operation status as read
        let status_lock = client.shared.status_lock.lock().unwrap();
        assert_eq!(status_lock.status, status::RUNNING);
        assert_eq!(status_lock.read, true);
    }

    /// Check that writing to the operation status works
    #[test]
    fn write() {
        // Send a status update
        let async_op = AsyncOp::new(status::PENDING);
        let (mut server, client) = async_op.split();
        server.update(status::RUNNING);

        // Check that it marks the operation status as unread
        let status_lock = client.shared.status_lock.lock().unwrap();
        assert_eq!(status_lock.status, status::RUNNING);
        assert_eq!(status_lock.read, false);
    }

    /// Check that waiting for status changes works
    #[test]
    fn wait() {
        // Create an asynchronous operation
        let async_op = AsyncOp::new(status::PENDING);
        let (mut server, mut client) = async_op.split();

        // Since this test involves blocking, we'll need another worker thread.
        // Setup some shared state so that we can synchronize with it.
        let shared_state = Arc::new(
            (Mutex::new(0), Condvar::new())
        );
        let worker_shared = shared_state.clone();

        // Create the worker thread, which will do all the waiting
        let worker = thread::spawn(move || {
            // Since the initial status is unread, the first wait should
            // return immediately to the caller
            let initial_status = client.wait();
            assert_eq!(initial_status, status::PENDING);

            // Tell the test code that we are still alive
            *worker_shared.0.lock().unwrap() = 1;
            worker_shared.1.notify_all();

            // Wait for the next operation status. This wait should block.
            let new_status = client.wait();

            // Tell the test code when we are done
            *worker_shared.0.lock().unwrap() = 2;
            worker_shared.1.notify_all();

            // Return the new operation status to the caller
            new_status
        });

        // The worker should be done with its first read quite quickly
        let wait_result = shared_state.1.wait_timeout(
            shared_state.0.lock().unwrap(),
            Duration::from_millis(100)
        );
        let (shared_lock, timeout_result) = wait_result.unwrap();
        assert!(!timeout_result.timed_out());
        assert_eq!(*shared_lock, 1);

        // Make sure that the worker does wait for a new status
        let wait_result = shared_state.1.wait_timeout(
            shared_lock,
            Duration::from_millis(100)
        );
        let (shared_lock, timeout_result) = wait_result.unwrap();
        assert!(timeout_result.timed_out());
        assert_eq!(*shared_lock, 1);

        // Send a status update to the worker
        server.update(status::DONE);

        // Make sure that the worker moves forward properly after that
        let wait_result = shared_state.1.wait_timeout(
            shared_lock,
            Duration::from_millis(100)
        );
        let (shared_lock, timeout_result) = wait_result.unwrap();
        assert!(!timeout_result.timed_out());
        assert_eq!(*shared_lock, 2);

        // Wait for the worker to complete and check its final status
        let new_status = worker.join().unwrap();
        assert_eq!(new_status, status::DONE);
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
