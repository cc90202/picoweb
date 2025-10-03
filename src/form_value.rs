use crate::utility::generate_html;
use crate::SharedSm2;
use core::cell::RefCell;

/// Guard RAII per sm2: garantisce che sm2 venga sempre disattivata,
/// anche in caso di panic durante la generazione dell'HTML.
struct Sm2Guard {
    shared_sm2: SharedSm2,
}

impl Sm2Guard {
    /// Crea un nuovo guard e attiva sm2
    /// Ritorna None se il lock fallisce (sm2 già in uso da altro task)
    fn new(shared_sm2: SharedSm2) -> Option<Self> {
        match shared_sm2.0.try_lock() {
            Ok(mut sm) => {
                sm.set_enable(true);
                log::info!("[Sm2Guard] sm2 attivata - inizia generazione HTML");
                Some(Self { shared_sm2 })
            }
            Err(_) => {
                log::warn!(
                    "[Sm2Guard] try_lock() fallito durante attivazione - \
                     sm2 già in uso da altro task o interrupt. \
                     La generazione HTML continuerà senza sm2 attiva."
                );
                None
            }
        }
    }
}

impl Drop for Sm2Guard {
    /// Disattiva sm2 quando il guard viene distrutto (anche in caso di panic)
    fn drop(&mut self) {
        match self.shared_sm2.0.try_lock() {
            Ok(mut sm) => {
                sm.set_enable(false);
                log::info!("[Sm2Guard] sm2 disattivata - fine generazione HTML");
            }
            Err(_) => {
                log::error!(
                    "[Sm2Guard::drop] try_lock() fallito durante disattivazione! \
                     sm2 potrebbe rimanere attiva. \
                     Possibile causa: sm2 bloccata da altro task o interrupt. \
                     ATTENZIONE: possibile stato inconsistente."
                );
            }
        }
    }
}

/// Form data structure (per la HTTP POST) per inserire le 9 righe
/// dello schema di Sudoku 9x9.
/// L'inserimento avviene ad esempio con: 5,3,_,_,7,_,_,_,_ e così via
/// per le 9 righe.
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
    /// Specifica il tipo di contenuto della risposta HTTP (HTML)
    fn content_type(&self) -> &'static str {
        "text/html"
    }

    /// Specifica la lunghezza del contenuto della risposta HTTP
    /// (utile per l'header Content-Length).
    /// Genera l'HTML attivando sm2 per la durata della generazione.
    /// Usa un guard RAII per garantire che sm2 venga sempre disattivata.
    ///
    /// # Ritorna
    /// * usize - Lunghezza del contenuto
    fn content_length(&self) -> usize {
        log::info!("[FormValue::content_length] Inizio generazione HTML");

        // Crea il guard RAII: sm2 viene attivata qui
        // e verrà automaticamente disattivata quando _guard esce dallo scope
        let _guard = match crate::get_shared_sm2() {
            Some(sm2) => Sm2Guard::new(sm2),
            None => {
                log::warn!(
                    "[FormValue::content_length] SharedSm2 non disponibile - \
                     non ancora inizializzato o errore di configurazione. \
                     Generazione HTML continuerà senza sm2."
                );
                None
            }
        };

        // Genera l'HTML con sm2 attiva (se guard è Some) e salva in self.message
        // Anche se generate_html va in panic, Drop verrà chiamato
        let html = generate_html(self);

        log::info!("[FormValue::content_length] Generazione HTML completata");

        // _guard viene droppato qui -> sm2 disattivata automaticamente
        html.as_bytes().content_length()
    }

    /// Ridefinisce il metodo per scrivere il contenuto della risposta HTTP in modo dinamico
    /// in base ai dati ricevuti nel form.
    /// Scrive il contenuto già generato in content_length (evita rigenerazione).
    ///
    /// # Argomenti
    /// * `writer` - Writer per scrivere il contenuto della risposta HTTP
    ///
    /// # Ritorna
    /// * Result<(), W::Error> - Risultato dell'operazione di scrittura
    async fn write_content<W: picoserve::io::Write>(self, mut writer: W) -> Result<(), W::Error> {
        log::info!("[FormValue::write_content] Inizio scrittura risposta HTTP");

        // Usa l'HTML già generato in content_length
        let content = self.message.borrow().clone();
        let content_len = content.len();

        let result = writer.write_all(content.as_str().as_bytes()).await;

        if result.is_ok() {
            log::info!(
                "[FormValue::write_content] Scrittura completata con successo ({} bytes)",
                content_len
            );
        } else {
            log::error!("[FormValue::write_content] Errore durante scrittura risposta HTTP");
        }

        result
    }
}
