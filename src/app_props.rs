//! # Application Router Configuration
//!
//! This module defines the application routes and HTTP endpoint handlers.

use crate::app_state::AppState;
use crate::form_value::FormValue;
use picoserve::AppWithStateBuilder;
use picoserve::routing::{PathRouter, get_service};

/// Application properties for configuring the web server routes.
pub struct AppProps;

impl AppWithStateBuilder for AppProps {
    type State = AppState;
    type PathRouter = impl PathRouter<AppState>;

    /// Builds the application router with defined endpoints.
    ///
    /// # Routes
    ///
    /// - `GET /` - Serves the main index page (index.html)
    /// - `GET /upload` - Serves the Sudoku form page (form.html)
    /// - `POST /upload` - Handles Sudoku puzzle submission and returns solution
    ///
    /// # Returns
    ///
    /// Configured router instance with all HTTP routes
    fn build_app(self) -> picoserve::Router<Self::PathRouter, Self::State> {
        picoserve::Router::new()
            .route(
                "/",
                get_service(picoserve::response::File::html(include_str!(
                    "../index.html"
                ))),
            )
            .route(
                "/upload",
                get_service(picoserve::response::File::html(include_str!(
                    "../form.html"
                )))
                .post(
                    |picoserve::extract::Form(form_value): picoserve::extract::Form<FormValue>| {
                        async move {
                            form_value
                        }
                    },
                ),
            )
    }
}
