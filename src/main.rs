//! Asynchronous operations (thread backend)
//!
//! This package is part of a long-term plan to build a general backend for the
//! implementation of asynchronous operations in Rust. As a first step, we'll
//! try to do it in a multi-threaded setting.

extern crate triple_buffer;

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
    Pending (Details::Pending),

    /// The request is being processed by the server
    Running (Details::Running),

    /// The server has successfully processed the request
    Done (Details::Done),

    /// The client has cancelled the request before the server was done
    Cancelled (Details::Cancelled),

    /// The server has failed to process the request
    Error (Details::Error),
}


/// TODO: Asynchronous operation status details


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
