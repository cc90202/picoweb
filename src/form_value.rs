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
    fn new(shared_sm2: SharedSm2) -> Option<Self> {
        if let Ok(mut sm) = shared_sm2.0.try_lock() {
            sm.set_enable(true);
            log::info!("sm2 attivata - inizia generazione HTML");
            Some(Self { shared_sm2 })
        } else {
            log::warn!("sm2 lock fallito - impossibile attivare sm2");
            None
        }
    }
}

impl Drop for Sm2Guard {
    /// Disattiva sm2 quando il guard viene distrutto (anche in caso di panic)
    fn drop(&mut self) {
        if let Ok(mut sm) = self.shared_sm2.0.try_lock() {
            sm.set_enable(false);
            log::info!("sm2 disattivata - fine generazione HTML");
        } else {
            log::error!("sm2 lock fallito durante drop - sm2 potrebbe rimanere attiva!");
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
        log::info!("CONTENT LENGTH - generazione HTML");

        // Crea il guard RAII: sm2 viene attivata qui
        // e verrà automaticamente disattivata quando _guard esce dallo scope
        let _guard = crate::get_shared_sm2().and_then(Sm2Guard::new);

        // Genera l'HTML con sm2 attiva e salva in self.message
        // Anche se generate_html va in panic, Drop verrà chiamato
        let html = generate_html(self);

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
        log::info!("WRITE CONTENT - usa HTML già generato");

        // Usa l'HTML già generato in content_length
        let content = self.message.borrow().clone();
        writer.write_all(content.as_str().as_bytes()).await
    }
}
