//! Multithreaded asynchronous operation monitoring
//!
//! This submodule provides facilities for asynchronous operation monitoring, in
//! situations where the worker is another thread running in the same OS process
//! as the asynchronous operation client.
//!
//! Three monitoring mechanisms are proposed:
//!
//! - Polling is suitable when a client is only interested in periodically
//!   checking the operation status and does not want to synchronize with status
//!   updates. One possible use case is refreshing UI controls.
//! - Blocking allows a client to wait for status updates. Although easy to use
//!   and reason about, this synchronization method should be used sparingly as
//!   it can have a strong averse effect on application performance
//! - Callbacks allow a client to schedule code to be executed whenever the
//!   operation status is updated. This is the most general and powerful
//!   synchronization mechanism, but it should be used with more care.


// TODO: Extract these modules to dedicated files
// TODO: Add callback-based implementation


/// Lock-free polling implementation of asynchronous operation monitoring
///
/// This implementation of the asynchronous operation concept is based on a
/// triple buffer. It does not allow waiting for updates, instead only allowing
/// the client to periodically poll the current operation status. But it may be
/// the most efficient option for frequently updated operation statuses.
///
pub mod polling {
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

            // Setup the client/server communication channel
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


    // TODO: Add tests and benchmarks
}


/// Blocking implementation of asynchronous operation monitoring
///
/// This implementation of the asynchronous operation concept uses a mutex to
/// synchronize access to the operation status. This limits the maximal
/// performance that can be achieved, but allows a client to wait for a status
/// updates in situations where this is desired.
///
pub mod blocking {
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
}
