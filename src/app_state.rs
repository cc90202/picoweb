use crate::SharedControl;

/// Stato dell'applicazione condifiviso tra i task embassy
pub struct AppState {
    pub shared_control: SharedControl,
}

// Permette di estrarre il controller condiviso dallo stato dell'applicazione
impl picoserve::extract::FromRef<AppState> for SharedControl {
    /// Ritorna il controller condiviso
    ///
    /// # Argomenti
    /// * `state` - Riferimento allo stato dell'applicazione
    ///
    /// # Ritorna
    /// * Self - Controller condiviso
    fn from_ref(state: &AppState) -> Self {
        state.shared_control
    }
}
