//! Asynchronous operations (thread backend)
//!
//! This package is part of a long-term plan to build a general backend for the
//! implementation of asynchronous operations in Rust. As a first step, we'll
//! try to do it in a multi-threaded setting.

extern crate triple_buffer;


// TODO: Extract independent concepts to dedicated code modules.


/// Lock-free polling implementation of asynchronous operation monitoring
///
/// This implementation of the asynchronous operation concept is based on a
/// triple buffer. It does not allow waiting for updates, instead only allowing
/// the client to periodically poll the current operation status. But it may be
/// the most efficient option for frequently updated operation statuses.
///
mod polling {
    use server::AsyncOpServer;
    use status::{AsyncOpStatus, AsyncOpStatusDetails};
    use triple_buffer::{TripleBuffer, TripleBufferOutput};


    /// Asynchronous operation object
    pub struct AsyncOp<StatusDetails: AsyncOpStatusDetails + 'static> {
        /// Server interface used to submit status updates
        server: AsyncOpServer<StatusDetails>,

        /// Client interface used to monitor the operation status
        client: AsyncOpClient<StatusDetails>,
    }
    //
    impl<StatusDetails: AsyncOpStatusDetails + 'static> AsyncOp<StatusDetails> {
        /// Create a new asynchronous operation object with some initial status
        pub fn new(initial_status: AsyncOpStatus<StatusDetails>) -> Self {
            // Keep a copy of the initial operation status
            let initial_status_copy = initial_status.clone();

            // Setup the client/server communication channel
            let buffer = TripleBuffer::new(initial_status);
            let (mut buf_input, buf_output) = buffer.split();

            // ...then build the server-side update hook...
            let updater = move | status: AsyncOpStatus<StatusDetails> | {
                // Update the value of the asynchronous operation status
                buf_input.write(status);
            };

            // ...then build the client and server
            AsyncOp {
                server: ::server::AsyncOpServer::new(updater,
                                                     &initial_status_copy),
                client: AsyncOpClient { buf_output: buf_output },
            }
        }

        /// Split the asynchronous operation object into client and server
        /// objects which can be respectively sent to client and server threads
        pub fn split(self) -> (AsyncOpServer<StatusDetails>,
                               AsyncOpClient<StatusDetails>) {
            (self.server, self.client)
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
    use server::AsyncOpServer;
    use status::{self, AsyncOpStatus, AsyncOpStatusDetails};
    use std::sync::{Arc, Mutex, Condvar};

    /// Asynchronous operation object
    pub struct AsyncOp<StatusDetails: AsyncOpStatusDetails + 'static> {
        /// Server interface used to submit status updates
        server: AsyncOpServer<StatusDetails>,

        /// Client interface used to monitor the operation status
        client: AsyncOpClient<StatusDetails>,
    }
    //
    impl<StatusDetails: AsyncOpStatusDetails + 'static> AsyncOp<StatusDetails> {
        /// Create a new asynchronous operation object with some initial status
        pub fn new(initial_status: AsyncOpStatus<StatusDetails>) -> Self {
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

            // ...then build the server-side update hook...
            let server_shared = shared_state.clone();
            let updater = move | status: AsyncOpStatus<StatusDetails> | {
                // Update the value of the asynchronous operation status
                *server_shared.status_lock
                              .lock()
                              .unwrap() = StatusWithDirtyBit { status: status,
                                                               read: false };

                // Notify the reader that an update has occured
                server_shared.update_cv.notify_all();
            };

            // ...then build the client and server
            AsyncOp {
                server: ::server::AsyncOpServer::new(updater,
                                                      &initial_status_copy),
                client: AsyncOpClient { shared: shared_state },
            }
        }

        /// Split the asynchronous operation object into client and server
        /// objects which can be respectively sent to client and server threads
        pub fn split(self) -> (AsyncOpServer<StatusDetails>,
                               AsyncOpClient<StatusDetails>) {
            (self.server, self.client)
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

    /// Server interface, used to submit status updates
    pub struct AsyncOpServer<Details: AsyncOpStatusDetails> {
        /// Functor that propagates a status update to the client
        updater: Box<FnMut(AsyncOpStatus<Details>)>,

        /// Flag indicating that the operation status has reached a final state
        /// and will not change anymore
        reached_final_status: bool,
    }
    //
    impl<Details: AsyncOpStatusDetails> AsyncOpServer<Details> {
        /// Create a new server interface with some initial status
        pub fn new<F>(updater: F,
                      initial_status: &AsyncOpStatus<Details>) -> Self
            where F: FnMut(AsyncOpStatus<Details>) + 'static
        {
            AsyncOpServer {
                updater: Box::new(updater),
                reached_final_status: status::is_final(initial_status),
            }
        }

        /// Update the current status of the asynchronous operation
        pub fn update(&mut self, status: AsyncOpStatus<Details>) {
            // This should only happen if we have not yet reached a final status
            debug_assert!(!self.reached_final_status);
            self.reached_final_status = status::is_final(&status);

            // Propagate the new operation status
            (self.updater)(status);
        }
    }
    //
    impl<Details: AsyncOpStatusDetails> Drop for AsyncOpServer<Details> {
        /// If the server is killed before the operation has reached its final
        /// status, notify the client in order to prevent hangs
        fn drop(&mut self) {
            if !self.reached_final_status {
                self.update(AsyncOpStatus::Error(AsyncOpError::ServerKilled));
            }
        }
    }


    // TODO: Add tests and benchmarks
}


/// Facilities to represent the status of asynchronous operations
mod status {
    use std::error::Error;
    use std::fmt::{self, Debug};

    /// Representation of an asynchronous operation's status
    ///
    /// This enumeration is used to represent the status of an asynchronous
    /// operation. It follows a state machine design, with customization points
    /// so that each application can add whichever extra information it needs.
    ///
    /// Here are the possible state transitions:
    ///
    /// - Pending -> Pending' / Running / Cancelled / Error
    /// - Running -> Running' / Done / Cancelled / Error
    ///
    /// Here, an apostrophe is used to denote a switch to another version of the
    /// same toplevel state with different implementation-specific details.
    ///
    /// Note that once the asynchronous operation is in either of the Done,
    /// Error or Cancelled state, its state won't change anymore.
    ///
    #[derive(Clone, Debug, PartialEq)]
    pub enum AsyncOpStatus<Details: AsyncOpStatusDetails> {
        /// The request has been submitted, but not been processed yet
        Pending(Details::PendingDetails),

        /// The request is being processed by the server
        Running(Details::RunningDetails),

        /// The server has successfully processed the request
        Done(Details::DoneDetails),

        /// The client has cancelled the request before the server was done
        Cancelled(Details::CancelledDetails),

        /// The server has failed to process the request
        Error(AsyncOpError<Details>),
    }
    //
    impl<Details: AsyncOpStatusDetails> AsyncOpStatusTraits
        for AsyncOpStatus<Details> {}


    /// Check if an asynchronous operation status is final
    pub fn is_final<Details: AsyncOpStatusDetails>(
        s: &AsyncOpStatus<Details>
    ) -> bool {
        use self::AsyncOpStatus::*;
        match *s {
            Pending(_) | Running (_) => false,
            Done(_) | Cancelled(_) | Error(_) => true,
        }
    }


    /// Asynchronous operation errors are described through the following enum
    #[derive(Clone, Debug, PartialEq)]
    pub enum AsyncOpError<Details: AsyncOpStatusDetails> {
        /// The server was killed before the operation reached a final status
        ServerKilled,

        /// An application-specific error has occurred
        CustomError(Details::ErrorDetails)
    }
    //
    impl<Details: AsyncOpStatusDetails> AsyncOpStatusTraits
        for AsyncOpError<Details> {}


    /// Every component of the asynchronous operation status should follow the
    /// following trait bounds
    pub trait AsyncOpStatusTraits: Clone + Debug + PartialEq + Send {}


    /// Implementation-specific details on the status of asynchronous operations
    /// are specified through an implementation of the following trait.
    pub trait AsyncOpStatusDetails: AsyncOpStatusTraits {
        /// Details on the status of pending operations
        ///
        /// Possible usage: Represent OpenCL's distinction between a command
        /// being submitted to the host driver, and queued on the device.
        ///
        type PendingDetails: AsyncOpStatusTraits;

        /// Details on the status of running operations
        ///
        /// Possible usage: Keep the client informed about server's progress.
        ///
        type RunningDetails: AsyncOpStatusTraits;

        /// Details on the status of completed operations
        ///
        /// Possible usage: Provide or give access to the operation's result.
        ///
        type DoneDetails: AsyncOpStatusTraits;

        /// Details on the status of cancelled operations
        ///
        /// Possible usage: Indicate why an operation was cancelled.
        ///
        type CancelledDetails: AsyncOpStatusTraits;

        /// Details on the status of erronerous operations
        ///
        /// Possible usage: Explain why the server could not perform the work.
        ///
        type ErrorDetails: AsyncOpStatusTraits + Error;
    }


    /// Placeholder for unneeded asynchronous operation details
    #[derive(Clone, Debug, PartialEq)]
    pub struct NoDetails {}
    //
    pub const NO_DETAILS: NoDetails = NoDetails {};
    //
    impl fmt::Display for NoDetails {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "<No details>")
        }
    }
    //
    impl Error for NoDetails {
        fn description(&self) -> &str {
            "<No error details>"
        }

        fn cause(&self) -> Option<&Error> {
            None
        }
    }
    //
    impl AsyncOpStatusTraits for NoDetails {}


    /// Fully standard asynchronous operation statuses, without any detail
    pub type StandardAsyncOpStatus = AsyncOpStatus<NoDetails>;
    //
    pub const PENDING: StandardAsyncOpStatus =
        AsyncOpStatus::Pending(NO_DETAILS);
    pub const RUNNING: StandardAsyncOpStatus =
        AsyncOpStatus::Running(NO_DETAILS);
    pub const DONE: StandardAsyncOpStatus =
        AsyncOpStatus::Done(NO_DETAILS);
    pub const CANCELLED: StandardAsyncOpStatus =
        AsyncOpStatus::Cancelled(NO_DETAILS);
    pub const SERVER_KILLED: StandardAsyncOpStatus =
        AsyncOpStatus::Error(AsyncOpError::ServerKilled);
    //
    impl AsyncOpStatusDetails for NoDetails {
        type PendingDetails = NoDetails;
        type RunningDetails = NoDetails;
        type DoneDetails = NoDetails;
        type CancelledDetails = NoDetails;
        type ErrorDetails = NoDetails;
    }
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
    assert_eq!(op_client.status(), status::SERVER_KILLED);
}
