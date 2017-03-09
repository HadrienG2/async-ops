//! Asynchronous operations (thread backend)
//!
//! This package is part of a long-term plan to build a general backend for the
//! implementation of asynchronous operations in Rust. As a first step, we'll
//! try to do it in a multi-threaded setting.

extern crate triple_buffer;


/// Lock-free implementation of asynchronous operations
///
/// This implementation of the asynchronous operation concept is based on a
/// triple buffer. It does not allow waiting for updates (which is racey and
/// bad for performance anyway), instead relying on an asynchronous callback
/// mechanism to notify the client about status updates. This allows much
/// improved performance at the cost of a less natural coding style.
///
mod lockfree {
    use status::{AsyncOpStatus, AsyncOpStatusDetails};
    use triple_buffer::{TripleBufferInput, TripleBufferOutput};

    /// Server interface, used to submit status updates
    pub struct AsyncOpServer<Details: AsyncOpStatusDetails> {
        /// Status updates will be submitted through this triple buffer
        buf_input: TripleBufferInput<AsyncOpStatus<Details>>,

        // TODO: Add callback support
    }

    /// Client interface, used to synchronize with the operation status
    pub struct AsyncOpClient<Details: AsyncOpStatusDetails> {
        /// Current operation status will be read through this triple buffer
        buf_output: TripleBufferOutput<AsyncOpStatus<Details>>,

        // TODO: Add callback support
    }
}


/// Lock-based implementation of asynchronous operations
///
/// This implementation of the asynchronous operation concept uses a mutex to
/// synchronize access to the operation status. This limits the maximal
/// performance that can be achieved, but allows a client to wait for a status
/// updates in the rare situations where this is desired.
///
mod locked {
    use status::{AsyncOpStatus, AsyncOpStatusDetails};
    use status::AsyncOpStatus::*;
    use status::AsyncOpError::*;
    use std::sync::{Arc, Mutex, Condvar};

    /// Asynchronous operation object
    pub struct AsyncOp<StatusDetails: AsyncOpStatusDetails> {
        server: AsyncOpServer<StatusDetails>,
        client: AsyncOpClient<StatusDetails>,
    }
    //
    impl<Details: AsyncOpStatusDetails> AsyncOp<Details> {
        /// Create a new asynchronous operation object with some initial status
        pub fn new(initial: AsyncOpStatus<Details>) -> Self {
            // Start with the shared state...
            let shared_state = Arc::new(
                SharedState {
                    status: Mutex::new(initial),
                    update_cv: Condvar::new(),
                }
            );

            // ...then build the client and server
            AsyncOp {
                server: AsyncOpServer { shared: shared_state.clone() },
                client: AsyncOpClient { shared: shared_state },
            }
        }

        /// Split the asynchronous operation object into client and server
        /// objects which can be respectively sent to client and server threads
        pub fn split(self) -> (AsyncOpServer<Details>, AsyncOpClient<Details>) {
            (self.server, self.client)
        }
    }


    /// Server interface, used to submit status updates
    pub struct AsyncOpServer<Details: AsyncOpStatusDetails> {
        shared: Arc<SharedState<Details>>,
    }
    //
    impl<Details: AsyncOpStatusDetails> AsyncOpServer<Details>
    {
        /// Submit an asynchronous operation status update
        pub fn update(&mut self, status: AsyncOpStatus<Details>) {
            // Update the value of the asynchronous operation status
            *self.shared.status.lock().unwrap() = status;

            // Notify the reader that an update has occured
            self.shared.update_cv.notify_all();
        }
    }
    //
    impl<Details: AsyncOpStatusDetails> Drop for AsyncOpServer<Details> {
        /// If the server is killed before the operation has reached its final
        /// status, notify the client in order to prevent hangs
        fn drop(&mut self) {
            // Check the final operation status
            let final_status = (*self.shared.status.lock().unwrap()).clone();
            match final_status {
                // Report early server exit as an error
                Pending(_) | Running(_)  => {
                    self.update(AsyncOpStatus::Error(ServerKilled));
                }
                // Otherwise, do nothing: everything is normal
                Done(_) | Cancelled(_) | Error(_) => {}
            }
        }
    }


    /// Client interface, used to synchronize with the operation status
    pub struct AsyncOpClient<Details: AsyncOpStatusDetails> {
        shared: Arc<SharedState<Details>>,
    }
    //
    impl<Details: AsyncOpStatusDetails> AsyncOpClient<Details>
    {
        /// Access the current asynchronous operation status
        pub fn status(&self) -> AsyncOpStatus<Details> {
            (*self.shared.status.lock().unwrap()).clone()
        }

        /// Wait for either a status update or a final operation status
        pub fn wait(&self) -> AsyncOpStatus<Details> {
            // Read the current operation status
            let mut status_lock = self.shared.status.lock().unwrap();
            match *status_lock {
                // If the status can still change, wait for a status update
                Pending(_) | Running(_)  => {
                    let wait_result = self.shared.update_cv.wait(status_lock);
                    status_lock = wait_result.unwrap();
                }
                // Otherwise, return the final operation status
                Done(_) | Cancelled(_) | Error(_) => {}
            }
            (*status_lock).clone()
        }
    }


    /// State shared between the client and the server
    struct SharedState<Details: AsyncOpStatusDetails> {
        status: Mutex<AsyncOpStatus<Details>>,
        update_cv: Condvar,
    }
}


/// Facilities to represent the status of asynchronous operations
mod status {
    use std::error::Error;
    use std::fmt;

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


    /// Asynchronous operation errors are described through the following enum
    #[derive(Clone, Debug, PartialEq)]
    pub enum AsyncOpError<Details: AsyncOpStatusDetails> {
        /// The server was killed before the operation reached a final status
        ServerKilled,

        /// An application-specific error has occurred
        CustomError(Details::ErrorDetails)
    }


    /// Implementation-specific details on the status of asynchronous operations
    pub trait AsyncOpStatusDetails: Clone + PartialEq + Send {
        /// Details on the status of pending operations
        ///
        /// Possible usage: Represent OpenCL's distinction between a command
        /// being submitted to the host driver, and queued on the device.
        ///
        type PendingDetails: Clone + PartialEq + Send;

        /// Details on the status of running operations
        ///
        /// Possible usage: Keep the client informed about server's progress.
        ///
        type RunningDetails: Clone + PartialEq + Send;

        /// Details on the status of completed operations
        ///
        /// Possible usage: Provide or give access to the operation's result.
        ///
        type DoneDetails: Clone + PartialEq + Send;

        /// Details on the status of cancelled operations
        ///
        /// Possible usage: Indicate why an operation was cancelled.
        ///
        type CancelledDetails: Clone + PartialEq + Send;

        /// Details on the status of erronerous operations
        ///
        /// Possible usage: Explain why the server could not perform the work.
        ///
        type ErrorDetails: Clone + PartialEq + Send + Error;
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


    /// Fully standard asynchronous operation status, without any detail
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
    let op = locked::AsyncOp::new(status::PENDING);

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
