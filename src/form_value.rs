//! # Form Handling and HTTP Response
//!
//! This module defines the form data structure for Sudoku puzzle submission
//! and implements the HTTP response generation with RAII-based resource management.

use crate::utility::generate_html;
use crate::SharedSm2;
use core::cell::RefCell;

/// RAII guard for State Machine 2.
///
/// Ensures SM2 is always disabled after use, even in case of panic during
/// HTML generation. SM2 is used only for debug/monitoring purposes and is
/// non-critical for application functionality.
struct Sm2Guard {
    shared_sm2: SharedSm2,
}

impl Sm2Guard {
    /// Creates a new guard and activates SM2.
    ///
    /// # Arguments
    ///
    /// * `shared_sm2` - Shared reference to State Machine 2
    ///
    /// # Returns
    ///
    /// * `Some(Self)` - If SM2 lock was acquired and SM2 was enabled
    /// * `None` - If SM2 is unavailable (non-critical, continues without debug timing)
    fn new(shared_sm2: SharedSm2) -> Option<Self> {
        match shared_sm2.0.try_lock() {
            Ok(mut sm) => {
                sm.set_enable(true);
                log::info!("SM2 activated for debug timing");
                Some(Self { shared_sm2 })
            }
            Err(_) => {
                log::debug!("SM2 unavailable - continuing without debug timing");
                None
            }
        }
    }
}

impl Drop for Sm2Guard {
    /// Disables SM2 when the guard is destroyed (even on panic).
    ///
    /// If disabling fails, logs a warning but does NOT panic since SM2 is
    /// only used for debug timing and is not critical for functionality.
    fn drop(&mut self) {
        if let Ok(mut sm) = self.shared_sm2.0.try_lock() {
            sm.set_enable(false);
            log::info!("SM2 disabled");
        } else {
            log::warn!("Unable to disable SM2 - non-critical (debug only)");
        }
    }
}

/// Form data structure for HTTP POST requests.
///
/// Contains the 9 rows of a 9x9 Sudoku puzzle. Each row is a comma-separated
/// string of values, with `_` representing empty cells.
///
/// # Example
///
/// ```text
/// row_1: "5,3,_,_,7,_,_,_,_"
/// row_2: "6,_,_,1,9,5,_,_,_"
/// ...
/// ```
#[derive(serde::Deserialize)]
pub struct FormValue {
    pub row_1: heapless::String<20>,
    pub row_2: heapless::String<20>,
    pub row_3: heapless::String<20>,
    pub row_4: heapless::String<20>,
    pub row_5: heapless::String<20>,
    pub row_6: heapless::String<20>,
    pub row_7: heapless::String<20>,
    pub row_8: heapless::String<20>,
    pub row_9: heapless::String<20>,
    #[serde(skip)]
    pub message: RefCell<heapless::String<1024>>,
}

impl picoserve::response::Content for FormValue {
    /// Returns the HTTP content type for the response.
    ///
    /// # Returns
    ///
    /// `"text/html"` content type string
    fn content_type(&self) -> &'static str {
        "text/html"
    }

    /// Calculates and returns the HTTP response content length.
    ///
    /// Generates the HTML response and activates SM2 for timing measurements
    /// during generation (if available). The RAII guard ensures SM2 is
    /// automatically disabled when this method returns.
    ///
    /// # Returns
    ///
    /// Content length in bytes
    fn content_length(&self) -> usize {
        // Create RAII guard: SM2 activated here, automatically disabled at end of scope
        let _guard = crate::get_shared_sm2().and_then(Sm2Guard::new);

        // Generate HTML and calculate length (SM2 remains active throughout)
        let html = generate_html(self);
        html.as_bytes().content_length()

        // _guard dropped at end of scope -> SM2 automatically disabled
    }

    /// Writes the HTTP response content dynamically based on form data.
    ///
    /// Uses the HTML already generated in `content_length()` to avoid
    /// regenerating the response content.
    ///
    /// # Arguments
    ///
    /// * `writer` - HTTP response writer
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If write succeeds
    /// * `Err(W::Error)` - If write fails
    async fn write_content<W: picoserve::io::Write>(self, mut writer: W) -> Result<(), W::Error> {
        // Use HTML already generated in content_length
        let content = self.message.borrow().clone();
        writer.write_all(content.as_str().as_bytes()).await
    }
}
