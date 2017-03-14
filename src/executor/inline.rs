//! Inline callback executor, implementing server-side callback execution
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
