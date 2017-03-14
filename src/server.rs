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
pub struct AsyncOpServer<Configuration: AsyncOpServerConfig> {
    /// User-configurable server behaviour
    config: Configuration,

    /// Flag indicating that the operation status has reached a final state
    /// and should not change anymore
    reached_final_status: bool,
}
//
impl<Config: AsyncOpServerConfig> AsyncOpServer<Config> {
    /// Create a new server interface with some initial status
    pub fn new(
        config: Config,
        initial_status: &AsyncOpStatus<Config::StatusDetails>
    ) -> Self {
        AsyncOpServer {
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
impl<Config: AsyncOpServerConfig> Drop for AsyncOpServer<Config> {
    /// If the server is killed before the operation has reached its final
    /// status, notify the client in order to prevent it from hanging
    fn drop(&mut self) {
        if !self.reached_final_status {
            self.update(AsyncOpStatus::Error(AsyncOpError::ServerKilled));
        }
    }
}


/// Configurable parameters and behaviour of AsyncOpServer
pub trait AsyncOpServerConfig {
    /// Implementation details of the asynchronous operation status
    type StatusDetails: AsyncOpStatusDetails;

    /// Method used to send status updates to the client
    fn update(&mut self, status: AsyncOpStatus<Self::StatusDetails>);
}


/// Unit tests
#[cfg(test)]
mod tests {
    use server::*;
    use status::{StandardAsyncOpStatus, NoDetails};
    use std::cell::RefCell;
    use std::rc::Rc;


    /// Check that servers are created in the right initial state
    #[test]
    fn initial_state() {
        // Test initial server state for a non-final status
        let pending_server = AsyncOpServer::new(
            MockServerConfig::new(status::PENDING),
            &status::PENDING
        );
        assert_eq!(*pending_server.config.last_status.borrow(), status::PENDING);
        assert_eq!(pending_server.config.update_count, 0);
        assert_eq!(pending_server.reached_final_status, false);

        // Test initial server state for a final status
        let final_server = AsyncOpServer::new(
            MockServerConfig::new(status::DONE),
            &status::DONE
        );
        assert_eq!(*final_server.config.last_status.borrow(), status::DONE);
        assert_eq!(final_server.config.update_count, 0);
        assert_eq!(final_server.reached_final_status, true);
    }


    /// Check that the server update() method works correctly
    #[test]
    fn correct_updates() {
        /// Start with a server in the pending state
        let mut server = AsyncOpServer::new(
            MockServerConfig::new(status::PENDING),
            &status::PENDING
        );

        /// Move it to the running state, check that it works
        server.update(status::RUNNING);
        assert_eq!(*server.config.last_status.borrow(), status::RUNNING);
        assert_eq!(server.config.update_count, 1);
        assert_eq!(server.reached_final_status, false);

        /// Move it to the done state, check that it works
        server.update(status::DONE);
        assert_eq!(*server.config.last_status.borrow(), status::DONE);
        assert_eq!(server.config.update_count, 2);
        assert_eq!(server.reached_final_status, true);
    }


    /// Check that trying to updating a final operation status will panic
    #[test]
    #[should_panic]
    fn incorrect_update() {
        /// Start with a server in a final state
        let mut server = AsyncOpServer::new(
            MockServerConfig::new(status::DONE),
            &status::DONE
        );

        /// Try to update it to another final state, this should fail
        server.update(status::ERROR_SERVER_KILLED);
    }


    /// Check that dropping a server is an error if and only if that server's
    /// associated asynchronous operation still has a non-final status.
    #[test]
    fn drop() {
        // We'll use this reference to keep track of the final operation status
        let mut final_status_ref: Rc<RefCell<StandardAsyncOpStatus>>;

        // Legitimate drop should work fine
        {
            let server = AsyncOpServer::new(
                MockServerConfig::new(status::DONE),
                &status::DONE
            );
            final_status_ref = server.config.last_status.clone();
        }
        assert_eq!(*final_status_ref.borrow(), status::DONE);

        // Dropping an operation with non-final status should be an error
        {
            let server = AsyncOpServer::new(
                MockServerConfig::new(status::RUNNING),
                &status::RUNNING
            );
            final_status_ref = server.config.last_status.clone();
        }
        assert_eq!(*final_status_ref.borrow(), status::ERROR_SERVER_KILLED);
    }


    /// Mock server config, suitable for unit testing
    struct MockServerConfig {
        /// Last status update sent by the server
        last_status: Rc<RefCell<StandardAsyncOpStatus>>,

        /// Number of status updates sent by the server so far
        update_count: i32,
    }
    //
    impl MockServerConfig {
        /// Create a new instance of the mock
        fn new(initial_status: StandardAsyncOpStatus) -> Self {
            MockServerConfig {
                last_status: Rc::new(RefCell::new(initial_status)),
                update_count: 0,
            }
        }
    }
    //
    impl AsyncOpServerConfig for MockServerConfig {
        /// Implementation details of the asynchronous operation status
        type StatusDetails = NoDetails;

        /// Method used to send status updates to the client
        fn update(&mut self, status: StandardAsyncOpStatus) {
            *self.last_status.borrow_mut() = status;
            self.update_count+= 1;
        }
    }
}
