use crate::app_state::AppState;
use crate::form_value::FormValue;
use picoserve::AppWithStateBuilder;
use picoserve::routing::{PathRouter, get_service};

pub struct AppProps;

// Costruisce il router dell'applicazione con le rotte definite
impl AppWithStateBuilder for AppProps {
    type State = AppState;
    type PathRouter = impl PathRouter<AppState>;

    /// Costruisce il router dell'applicazione con gli endpoint.
    ///
    /// # Ritorna
    /// * picoserve::Router<Self::PathRouter, Self::State>
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
