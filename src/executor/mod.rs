//! Asynchronous callback executors
//!
//! Whenever callback-based asynchronous notifications are used, one important
//! design issue is to decide how the callback functions should be executed.
//!
//! A traditional answer to this problem has been to run callbacks directly on
//! the asynchronous operation server. While this approach works and has minimal
//! scheduling overhead, it also has a number of issues that prevent it from
//! being universally applicable:
//!
//! - Long-running callbacks can have a harmful impact on server performance
//! - In distributed scenarios where the client and the server live in separate
//!   hardware spaces, the callback may have a hard time accessing the client
//! - And in such distributed scenarios, a server-side callback also raises
//!   serious security issues, since it allows code injection attacks
//!
//! For this reason, we would rather have a client-side component which is in
//! charge of receiving status update notifications from the server and making
//! sure that the appropriate callback get executed on the client side. For
//! consistency with the terminology of C++ tasking runtimes, we will call this
//! component a callback executor, or executor for short.

pub mod inline;
// TODO: Add thread pool executor

use status::{AsyncOpStatus, AsyncOpStatusDetails};


/// A callback executor is in charge of making sure that a client-specified
/// callback is executed whenever an operation status update is received
pub trait CallbackExecutor {
    /// Notification channel used by the server to tell the client about updates
    ///
    /// TODO: Once associated type constructors land in Rust, avoid type erasure
    ///       issues and allow non-static callback lifetimes by switching to a
    ///       CallbackChannel<'a, Details> type family
    ///
    type Channel: AnyCallbackChannel;

    /// Setup an asynchronous notification channel with a certain callback
    fn setup_callback<F, Details>(&mut self, callback: F) -> Self::Channel
        where F: Fn(AsyncOpStatus<Details>) + 'static,
              Details: AsyncOpStatusDetails + 'static;
}


/// A callback channel is used by the server to send a status update to the
/// client, making sure that associated callbacks get executed
pub trait CallbackChannel<'a, Details: AsyncOpStatusDetails> {
    /// Notify the client that an operation status update has occured
    fn notify(&mut self, new_status: AsyncOpStatus<Details>);
}


/// Type-erased variant of CallbackChannel, used as a temporary workaround until
/// associated type constructors land in Rust
///
/// TODO: Deprecate this once associated type constructors land in Rust.
///
pub trait AnyCallbackChannel {
    /// Check if the callback channel was configured for the right status type
    fn is_compatible<Details>(&self) -> bool
        where Details: AsyncOpStatusDetails + 'static;

    /// Attempt to notify the client about a status update, will panic if the
    /// incorrect status details are specified
    fn notify<Details>(&mut self, new_status: AsyncOpStatus<Details>)
        where Details: AsyncOpStatusDetails + 'static;
}
