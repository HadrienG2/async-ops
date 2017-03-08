//! Asynchronous operations (thread backend)
//!
//! This package is part of a long-term plan to build a general backend for the
//! implementation of asynchronous operations in Rust. As a first step, we'll
//! try to do it in a multi-threaded setting.

extern crate triple_buffer;

use std::error::Error;
use std::fmt;
use triple_buffer::TripleBuffer;


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
/// Note that once the asynchronous operation is in either of the Done, Error or
/// Cancelled state, its state won't change anymore.
///
enum AsyncOpStatus<Details: AsyncOpStatusDetails> {
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
trait AsyncOpStatusDetails {
    /// Details on the status of pending operations. For example, OpenCL
    /// distinguishes commands which are queued on the host and on the device.
    type PendingDetails: Clone + PartialEq + Send;

    /// Details on the status of running operations. A typical application is
    /// tracking progress using unsigned integer counters.
    type RunningDetails: Clone + PartialEq + Send;

    /// Details on the status of completed operations. Can be used to store a
    /// handle to operation results, for example.
    type DoneDetails: Clone + PartialEq + Send;

    /// Details on the status of cancelled operations. Can be used to provide
    /// details on why a client has cancelled some operation.
    type CancelledDetails: Clone + PartialEq + Send;

    /// Details on the status of erronerous operations. Can be used by the
    /// server to explain why it could not complete its work.
    type ErrorDetails: Clone + PartialEq + Send + Error;
}


/// If the server has no errors to repport, you can use this for error details
#[derive(Clone, Debug, PartialEq)]
struct NoErrorDetails {}
//
impl fmt::Display for NoErrorDetails {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<No error details>")
    }
}
//
impl Error for NoErrorDetails {
    fn description(&self) -> &str {
        "<No error details>"
    }

    fn cause(&self) -> Option<&Error> {
        None
    }
}


/// If the standard asynchronous operation status is enough for you, you don't
/// need to use the AsyncOpStatusDetails feature
type StandardAsyncOpStatus = AsyncOpStatus<()>;
//
impl AsyncOpStatusDetails for () {
    type PendingDetails = ();
    type RunningDetails = ();
    type DoneDetails = ();
    type CancelledDetails = ();
    type ErrorDetails = NoErrorDetails;
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
