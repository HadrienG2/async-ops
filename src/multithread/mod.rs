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


pub mod blocking;
// TODO: Add callback-based implementation
pub mod polling;
