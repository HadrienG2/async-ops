//! Asynchronous operations (thread backend)
//!
//! This package is part of a long-term plan to build a general backend for the
//! implementation of asynchronous operations in Rust. As a first step, we'll
//! try to do it in a multi-threaded setting.

extern crate triple_buffer;

use triple_buffer::TripleBuffer;


/// Common interface to all asynchronous operations
mod interface {
    /// An asynchronous operation server can submit status updates
    trait AsyncOpServer<StatusDetails: ::status::AsyncOpStatusDetails> {
        /// Update the asynchronous operation status
        fn update(&mut self, status: ::status::AsyncOpStatus<StatusDetails>);
    }

    /// An asynchronous operation client can poll the operation status, and
    /// synchronize with it in implementation-dependent ways
    trait AsyncOpClient<StatusDetails: ::status::AsyncOpStatusDetails> {
        /// Access the current asynchronous operation status
        fn status(&mut self) -> &::status::AsyncOpStatus<StatusDetails>;
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
        Error(Details::ErrorDetails),
    }


    /// Implementation-specific details on the status of asynchronous operations
    pub trait AsyncOpStatusDetails {
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
    pub const ERROR: StandardAsyncOpStatus =
        AsyncOpStatus::Error(NO_DETAILS);
    //
    impl AsyncOpStatusDetails for NoDetails {
        type PendingDetails = NoDetails;
        type RunningDetails = NoDetails;
        type DoneDetails = NoDetails;
        type CancelledDetails = NoDetails;
        type ErrorDetails = NoDetails;
    }
}


// For now, this is just a copy/paste of the TripleBuffer usage example
fn main() {
    // Create a triple buffer of any Clone type:
    let buf = TripleBuffer::new(0);

    // Split it into an input and output interface, to be respectively sent to
    // the producer thread and the consumer thread:
    let (mut buf_input, mut buf_output) = buf.split();

    // The producer can move a value into the buffer at any time
    buf_input.write(42);

    // The consumer can access the latest value from the producer at any time
    let latest_value_ref = buf_output.read();
    assert_eq!(*latest_value_ref, 42);
}
