//! Inline callback executor, implementing synchronous callback execution
//!
//! This callback executor follows the traditional pattern of directly executing
//! callbacks on the server side. It can harm server performance, and for
//! security reasons should never be allowed to cross a process or machine
//! boundary, but in local communication perimeters like coroutines and threads
//! it can be a good choice for short performance-critical callbacks

use executor::{CallbackExecutor, CallbackChannel, AnyCallbackChannel};
use status::{AsyncOpStatus, AsyncOpStatusDetails};
use std::any::Any;


/// CallbackExecutor implementation suitable for inline callback execution
pub struct InlineCallbackExecutor {}
//
impl InlineCallbackExecutor {
    /// Create a new inline callback executor
    pub fn new() -> Self {
        InlineCallbackExecutor {}
    }
}
//
impl CallbackExecutor for InlineCallbackExecutor {
    type Channel = AnyInlineCallbackChannel;

    fn setup_callback<F, Details>(&mut self, callback: F) -> Self::Channel
        where F: Fn(AsyncOpStatus<Details>) + 'static,
              Details: AsyncOpStatusDetails + 'static
    {
        AnyInlineCallbackChannel {
            holder: Box::new(
                InlineCallbackChannel {
                    callback: Box::new(callback)
                }
            )
        }
    }
}


/// Callback channel which invokes an internal callback whenever a new operation
/// status is pushed into it
pub struct InlineCallbackChannel<'a, Details: AsyncOpStatusDetails> {
    callback: Box<Fn(AsyncOpStatus<Details>) + 'a>,
}
//
impl<'a, Details: AsyncOpStatusDetails> CallbackChannel<'a, Details>
    for InlineCallbackChannel<'a, Details>
{
    fn notify(&mut self, new_status: AsyncOpStatus<Details>) {
        (self.callback)(new_status);
    }
}


/// AnyCallbackChannel implementation corresponding to InlineCallbackChannel
pub struct AnyInlineCallbackChannel {
    holder: Box<Any>,
}
//
impl AnyCallbackChannel for AnyInlineCallbackChannel {
    fn is_compatible<Details>(&self) -> bool
        where Details: AsyncOpStatusDetails + 'static
    {
        self.holder.is::<InlineCallbackChannel<Details>>()
    }

    fn notify<Details>(&mut self, new_status: AsyncOpStatus<Details>)
        where Details: AsyncOpStatusDetails + 'static
    {
        let mut channel = self.holder
                              .downcast_mut::<InlineCallbackChannel<Details>>()
                              .unwrap();
        channel.notify(new_status);
    }
}


/// Unit tests
#[cfg(test)]
mod tests {
    use executor::inline::*;
    use status::{self, NoDetails, StandardAsyncOpStatus};
    use std::cell::Cell;
    use std::rc::Rc;

    // Make sure that executor creation works well
    #[test]
    fn new_executor() {
        let _ = InlineCallbackExecutor::new();
    }

    // Make sure that callback channels are set up properly
    #[test]
    #[allow(unused_variables)]
    fn callback_setup() {
        // This callback will set a boolean flag if called
        let called = Rc::new(Cell::new(false));
        let c_called = called.clone();
        let callback = move | s: StandardAsyncOpStatus | c_called.set(true);

        // Setup a callback channel for it
        let mut executor = InlineCallbackExecutor::new();
        let channel = executor.setup_callback(callback);

        // Check that the right type of callback channel was created
        assert!(channel.is_compatible::<NoDetails>());

        // Check that the callback was not called during setup
        assert!(!called.get());
    }

    // Make sure that callback channels propagate updates as expected
    #[test]
    fn update() {
        // This callback will increment a counter if called
        let counter = Rc::new(Cell::new(0));
        let c_counter = counter.clone();
        let callback = move | s: StandardAsyncOpStatus | {
            assert_eq!(s, status::DONE);
            c_counter.set(c_counter.get() + 1);
        };

        // Setup a callback channel for it
        let mut executor = InlineCallbackExecutor::new();
        let mut channel = executor.setup_callback(callback);

        // Check that the callback gets called exactly once on status updates
        channel.notify(status::DONE);
        assert_eq!(counter.get(), 1);
    }
}


// TODO: Add benchmarks
