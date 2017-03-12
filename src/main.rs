//! Asynchronous operations (thread backend)
//!
//! This package is part of a long-term plan to build a general backend for the
//! implementation of asynchronous operations in Rust. As a first step, we'll
//! try to do it in a multi-threaded setting.

extern crate triple_buffer;

mod status;


// TODO: Extract independent concepts to dedicated code modules.


/// Lock-free polling implementation of asynchronous operation monitoring
///
/// This implementation of the asynchronous operation concept is based on a
/// triple buffer. It does not allow waiting for updates, instead only allowing
/// the client to periodically poll the current operation status. But it may be
/// the most efficient option for frequently updated operation statuses.
///
mod polling {
    use server::{GenericAsyncOpServer, AsyncOpUpdater};
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

            // Setup the client/server communication channel
            let buffer = TripleBuffer::new(initial_status);
            let (buf_input, buf_output) = buffer.split();

            // ...then build the client and server
            AsyncOp {
                server: GenericAsyncOpServer::new(
                    AsyncOpUpdaterImpl { buf_input: buf_input },
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
        GenericAsyncOpServer<Details, AsyncOpUpdaterImpl<Details>>;


    /// Mechanism through which status updates should be sent to the client
    pub struct AsyncOpUpdaterImpl<Details: AsyncOpStatusDetails> {
        /// New operation statuses will be sent through this tripe buffer
        buf_input: TripleBufferInput<AsyncOpStatus<Details>>,
    }
    //
    impl<Details: AsyncOpStatusDetails> AsyncOpUpdater<Details>
        for AsyncOpUpdaterImpl<Details>
    {
        /// Send a status update to the client
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


    // TODO: Add tests and benchmarks
}


/// Blocking implementation of asynchronous operation monitoring
///
/// This implementation of the asynchronous operation concept uses a mutex to
/// synchronize access to the operation status. This limits the maximal
/// performance that can be achieved, but allows a client to wait for a status
/// updates in situations where this is desired.
///
mod blocking {
    use server::{GenericAsyncOpServer, AsyncOpUpdater};
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
                        StatusWithDirtyBit {
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
                    AsyncOpUpdaterImpl { shared: shared_state.clone() },
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
        GenericAsyncOpServer<Details, AsyncOpUpdaterImpl<Details>>;


    /// Mechanism through which status updates should be sent to the client
    pub struct AsyncOpUpdaterImpl<Details: AsyncOpStatusDetails> {
        /// Reference-counted shared state
        shared: Arc<SharedState<Details>>,
    }
    //
    impl<Details: AsyncOpStatusDetails> AsyncOpUpdater<Details>
        for AsyncOpUpdaterImpl<Details>
    {
        /// Send a status update to the client
        fn update(&mut self, status: AsyncOpStatus<Details>) {
            // Update the value of the asynchronous operation status
            *self.shared.status_lock
                        .lock()
                        .unwrap() = StatusWithDirtyBit { status: status,
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
        status_lock: Mutex<StatusWithDirtyBit<Details>>,

        /// Condition variable used to notify clients about status updates
        update_cv: Condvar,
    }
    //
    struct StatusWithDirtyBit<Details: AsyncOpStatusDetails> {
        /// Current asynchronous operation status
        status: AsyncOpStatus<Details>,

        /// Whether this version of the status was read by the client
        read: bool,
    }


    // TODO: Add tests and benchmarks
}


/// General implementation of an asynchronous operation server
///
/// This is a fully general implementation of an asynchronous operation server,
/// suitable no matter which server-side behavior is desired by the client.
/// However, the raw abstraction should not be directly exposed to clients, as
/// it would allow arbitrary server code injection.
///
mod server {
    use status::{self, AsyncOpError, AsyncOpStatus, AsyncOpStatusDetails};
    use std::marker::PhantomData;

    /// Server interface, used to submit status updates
    pub struct GenericAsyncOpServer<Details: AsyncOpStatusDetails,
                                    StatusUpdater: AsyncOpUpdater<Details>> {
        /// Configurable mechanism for propagating status updates to the client
        updater: StatusUpdater,

        /// Flag indicating that the operation status has reached a final state
        /// and will not change anymore
        reached_final_status: bool,

        /// This crap is needed due to an incorrect occurrence of E0392
        yes_i_use_details: PhantomData<Details>,
    }
    //
    impl<Details: AsyncOpStatusDetails,
         StatusUpdater: AsyncOpUpdater<Details>>
    GenericAsyncOpServer<Details, StatusUpdater> {
        /// Create a new server interface with some initial status
        pub fn new(status_updater: StatusUpdater,
                   initial_status: &AsyncOpStatus<Details>) -> Self {
            GenericAsyncOpServer {
                updater: status_updater,
                reached_final_status: status::is_final(initial_status),
                yes_i_use_details: PhantomData,
            }
        }

        /// Update the current status of the asynchronous operation
        pub fn update(&mut self, status: AsyncOpStatus<Details>) {
            // This should only happen if we have not yet reached a final status
            debug_assert!(!self.reached_final_status);
            self.reached_final_status = status::is_final(&status);

            // Propagate the new operation status
            self.updater.update(status);
        }
    }
    //
    impl<Details: AsyncOpStatusDetails,
         StatusUpdater: AsyncOpUpdater<Details>>
    Drop for GenericAsyncOpServer<Details, StatusUpdater> {
        /// If the server is killed before the operation has reached its final
        /// status, notify the client in order to prevent hangs
        fn drop(&mut self) {
            if !self.reached_final_status {
                self.update(AsyncOpStatus::Error(AsyncOpError::ServerKilled));
            }
        }
    }


    /// Configurable parameter of GenericAsyncOpServer specifying how status
    /// updates should be propagated to the asynchronous operation's client
    pub trait AsyncOpUpdater<Details: AsyncOpStatusDetails> {
        /// Send a status update to the client
        fn update(&mut self, status: AsyncOpStatus<Details>);
    }


    // TODO: Add tests and benchmarks
}


fn main() {
    // Create an asynchronous operation
    let op = blocking::AsyncOp::new(status::PENDING);

    // Split it into a client and a server
    let (op_server, op_client) = op.split();

    // Check initial status
    println!("Initial operation status is {:?}", op_client.status());
    {
        // Update the status, then drop the server
        let mut server = op_server;
        server.update(status::RUNNING);
        println!("New operation status is {:?}", op_client.status());
    }

    // Check final status
    println!("Final operation status is {:?}", op_client.status());
    assert_eq!(op_client.status(), status::ERROR_SERVER_KILLED);
}
