//! # Application State Management
//!
//! This module defines the shared application state accessible to all HTTP handlers.

use crate::SharedControl;

/// Application state shared across Embassy tasks and HTTP handlers.
///
/// Contains resources that need to be accessible to web request handlers,
/// such as the WiFi controller for LED control and status checks.
pub struct AppState {
    /// Shared WiFi controller for CYW43 chip operations
    pub shared_control: SharedControl,
}

impl picoserve::extract::FromRef<AppState> for SharedControl {
    /// Extracts the shared WiFi controller from application state.
    ///
    /// This implementation allows HTTP handlers to access the WiFi controller
    /// through picoserve's dependency injection system.
    ///
    /// # Arguments
    ///
    /// * `state` - Reference to the application state
    ///
    /// # Returns
    ///
    /// Copy of the shared WiFi controller wrapper
    fn from_ref(state: &AppState) -> Self {
        state.shared_control
    }
}
