//! Facilities to represent the status of asynchronous operations
//!
//! This package provides facilities to represent and reason about the status
//! of asynchronous operations. The model is the following: any kind of async
//! operation can be represented as a state machine which starts on the client
//! side in a pending state, is hopefully scheduled and run on the server, goes
//! through a number of intermediary "running" stats in this process, and
//! finally ends up in a successful of unsuccessful final state.

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


/// Check if an operation status is final (i.e. won't change anymore)
pub fn is_final<Details: AsyncOpStatusDetails>(
    s: &AsyncOpStatus<Details>
) -> bool {
    use self::AsyncOpStatus::*;
    match *s {
        Pending(_) | Running (_) => false,
        Done(_) | Cancelled(_) | Error(_) => true,
    }
}


/// Support for standard and custom asynchronous operation errors
#[derive(Clone, Debug, PartialEq)]
pub enum AsyncOpError<Details: AsyncOpStatusDetails> {
    /// The server was killed before the operation reached a final status
    ServerKilled,

    /// An application-specific error has occurred
    #[allow(dead_code)]
    CustomError(Details::ErrorDetails)
}
//
impl<Details: AsyncOpStatusDetails> AsyncOpStatusTraits
    for AsyncOpError<Details> {}


/// Trait bounds which every part of the operation status should honor
pub trait AsyncOpStatusTraits: Clone + Debug + PartialEq + Send {}


/// Implementation-specific details on the status of asynchronous operations
pub trait AsyncOpStatusDetails: AsyncOpStatusTraits {
    /// Details on the status of pending operations
    ///
    /// Possible usage: Represent OpenCL's distinction between a command
    /// being submitted to the host driver, and queued on the device.
    ///
    type PendingDetails: AsyncOpStatusTraits;

    /// Details on the status of running operations
    ///
    /// Possible usage: Keep the client informed about the server's progress.
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
pub const ERROR_SERVER_KILLED: StandardAsyncOpStatus =
    AsyncOpStatus::Error(AsyncOpError::ServerKilled);
//
impl AsyncOpStatusDetails for NoDetails {
    type PendingDetails = NoDetails;
    type RunningDetails = NoDetails;
    type DoneDetails = NoDetails;
    type CancelledDetails = NoDetails;
    type ErrorDetails = NoDetails;
}


/// Unit tests
#[cfg(test)]
mod tests {
    use ::status::*;

    /// Test that the standard asynchronous operation statuses match their
    /// more general generic cousins
    #[test]
    fn test_standard_statuses() {
        // Standard "pending" status
        match PENDING {
            AsyncOpStatus::Pending(_) => {},
            _ => panic!("PENDING status is incorrectly defined"),
        }
        assert!(!is_final(&PENDING));

        // Standard "running" status
        match RUNNING {
            AsyncOpStatus::Running(_) => {},
            _ => panic!("RUNNING status is incorrectly defined"),
        }
        assert!(!is_final(&RUNNING));

        // Standard "done" status
        match DONE {
            AsyncOpStatus::Done(_) => {},
            _ => panic!("DONE status is incorrectly defined"),
        }
        assert!(is_final(&DONE));

        // Standard "cancelled" status
        match CANCELLED {
            AsyncOpStatus::Cancelled(_) => {},
            _ => panic!("CANCELLED status is incorrectly defined"),
        }
        assert!(is_final(&CANCELLED));

        // Standard "error" status
        match ERROR_SERVER_KILLED {
            AsyncOpStatus::Error(AsyncOpError::ServerKilled) => {},
            _ => panic!("ERROR_SERVER_KILLED status is incorrectly defined"),
        }
        assert!(is_final(&ERROR_SERVER_KILLED));
    }
}
