//! General implementation of an asynchronous operation server
//!
//! This module contains a generic implementation of an asynchronous operation
//! server, suitable for sending asynchronous operation status updates to a
//! client no matter where the client is located and how it plans on
//! synchronizing with the server.
//!
//! Note that in general, this raw abstraction should not be directly exposed to
//! clients, as doing so would allow arbitrary server code injection.

use status::{self, AsyncOpError, AsyncOpStatus, AsyncOpStatusDetails};


/// Server interface, used to submit asynchronous operation status updates
pub struct GenericAsyncOpServer<Configuration: AsyncOpServerConfig> {
    /// User-configurable server behaviour
    config: Configuration,

    /// Flag indicating that the operation status has reached a final state
    /// and should not change anymore
    reached_final_status: bool,
}
//
impl<Config: AsyncOpServerConfig> GenericAsyncOpServer<Config> {
    /// Create a new server interface with some initial status
    pub fn new(
        config: Config,
        initial_status: &AsyncOpStatus<Config::StatusDetails>
    ) -> Self {
        GenericAsyncOpServer {
            config: config,
            reached_final_status: status::is_final(initial_status),
        }
    }

    /// Update the current status of the asynchronous operation
    pub fn update(&mut self,
                  status: AsyncOpStatus<Config::StatusDetails>) {
        // This should only happen if we have not yet reached a final status
        debug_assert!(!self.reached_final_status);
        self.reached_final_status = status::is_final(&status);

        // Propagate the new operation status
        self.config.update(status);
    }
}
//
impl<Config: AsyncOpServerConfig> Drop for GenericAsyncOpServer<Config> {
    /// If the server is killed before the operation has reached its final
    /// status, notify the client in order to prevent it from hanging
    fn drop(&mut self) {
        if !self.reached_final_status {
            self.update(AsyncOpStatus::Error(AsyncOpError::ServerKilled));
        }
    }
}


/// Configurable parameters of GenericAsyncOpServer
pub trait AsyncOpServerConfig {
    /// Implementation details of the asynchronous operation status
    type StatusDetails: AsyncOpStatusDetails;

    /// Method used to send status updates to the client
    fn update(&mut self, status: AsyncOpStatus<Self::StatusDetails>);
}


// TODO: Add tests
