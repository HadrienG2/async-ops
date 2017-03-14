//! Minimal asynchronous operation client interface
//!
//! Generally speaking, the details of the asynchronous operation client
//! interface depend on the specifics of how a client has chosen to monitor and
//! synchronize with the asynchronous operation status. However, one service
//! which is common to all asynchronous operation clients is the ability to
//! request the cancellation of an asynchronous operation.
//!
//! Note that the precise semantics of cancellation are application-specific.
//! And since some asynchronous programming backends do not natively support
//! cancellation, we cannot even guarantee that the request will always be
//! honored in specific circumstances, such as when an operation is pending.
//! What we can guarantee, however, is that a server is able to check at any
//! time whether a cancellation request has been sent by the client.
//!
//! It is highly recommended that implementors of asynchronous operation servers
//! periodically check such cancellation requests, and adjust their behaviour
//! accordingly by performing early termination, whenever reasonable feasible.


/// Features which all asynchronous operation clients are expected to share
pub trait IAsyncOpClient {
    /// Request the cancellation of the active asynchronous operation
    fn cancel(&mut self);
}
