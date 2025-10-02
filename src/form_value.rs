use crate::utility::generate_html;
use core::cell::RefCell;

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
    ///
    /// # Ritorna
    /// * &'static str - Tipo di contenuto
    fn content_type(&self) -> &'static str {
        "text/html"
    }

    /// Specifica la lunghezza del contenuto della risposta HTTP
    /// (utile per l'header Content-Length).
    ///
    /// # Ritorna
    /// * usize - Lunghezza del contenuto
    fn content_length(&self) -> usize {
        log::info!("CONTENT LENGTH");
        let html = generate_html(self);
        html.as_bytes().content_length()
    }

    /// Ridefinisce il metodo per scrivere il contenuto della risposta HTTP in modo dinamico
    /// in base ai dati ricevuti nel form.
    ///
    /// # Argomenti
    /// * `writer` - Writer per scrivere il contenuto della risposta HTTP
    ///
    /// # Ritorna
    /// * Result<(), W::Error> - Risultato dell'operazione di scrittura
    async fn write_content<W: picoserve::io::Write>(self, mut writer: W) -> Result<(), W::Error> {
        log::info!("WRITE CONTENT");

        // per essere sicuri che sia dropped dopo await
        let content = self.message.borrow().clone();
        writer.write_all(content.as_str().as_bytes()).await
    }
}
