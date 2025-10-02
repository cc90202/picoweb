//! Funzioni di utility per la generazione di pagine HTML
use crate::configuration::CONFIG;
use crate::form_value::FormValue;
use crate::sudoku::Sudoku;
use heapless::Vec;
use heapless::format;

const HTML_HEADER: &str =
    "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Sudoku Result</title></head><body>";
const HTML_FOOTER: &str = "</body></html>";

#[macro_export]
macro_rules! panic_led_loop {
    ($control:expr) => {
        if let Some(panic_message) = panic_persist::get_panic_message_utf8() {
            let _ = $control.gpio_set(0, true).await;
            log::error!("{panic_message}");
            loop {
                embassy_time::Timer::after_secs(5).await;
            }
        }
    };
}

#[macro_export]
macro_rules! panic_led_loop_shared {
    ($shared_control:expr) => {
        if let Some(panic_message) = panic_persist::get_panic_message_utf8() {
            let mut control = $shared_control.0.lock().await;
            let _ = control.gpio_set(0, true).await;
            log::error!("{panic_message}");
            loop {
                embassy_time::Timer::after_secs(5).await;
            }
        }
    };
}

// Genera una pagina HTML di errore.
///
/// # Argomenti
/// * `msg` - Messaggio di errore
///
/// # Ritorna
/// * heapless::String<1024> - Pagina HTML generata
pub fn error_html<T: core::fmt::Debug>(msg: &str, err: &T) -> heapless::String<1024> {
    format!(
        "{header}<h1>{msg}: {:?}</h1>{footer}",
        err,
        header = HTML_HEADER,
        footer = HTML_FOOTER
    )
    .unwrap_or_default()
}

/// Genera una tabella HTML dal risultato del Sudoku risolto.
///
/// # Argomenti
/// * `grid` - Riferimento alla matrice 9x9 del Sudoku risolto
///
/// # Ritorna
/// * heapless::String<1024> - Tabella HTML generata
pub fn html_table(grid: &[[u8; 9]; 9]) -> heapless::String<1024> {
    let mut html = heapless::String::<1024>::new();
    html.push_str(HTML_HEADER).unwrap();
    html.push_str("<h1>Solved Sudoku</h1><table border=\"1\">")
        .unwrap();
    for row in grid.iter() {
        html.push_str("<tr>").unwrap();
        for cell in row.iter() {
            let s: heapless::String<24> = format!("<td>{cell}</td>").unwrap_or_default();
            html.push_str(s.as_str()).unwrap_or_default();
        }
        html.push_str("</tr>").unwrap_or_default();
    }
    html.push_str("</table>").unwrap_or_default();
    html.push_str(HTML_FOOTER).unwrap_or_default();
    html
}

/// Estrae l'SSID dalla configurazione.
///
/// # Ritorna
/// * &str - SSID della rete WiFi
pub fn get_ssid() -> &'static str {
    CONFIG
        .lines()
        .find(|line| line.starts_with("WIFI_NETWORK="))
        .map(|line| &line["WIFI_NETWORK=".len()..])
        .unwrap_or("")
}

/// Estrae la password di rete dalla configurazione.
///
/// # Ritorna
/// * &str - Password della rete WiFi
pub fn get_wifi_password() -> &'static str {
    CONFIG
        .lines()
        .find(|line| line.starts_with("WIFI_PASSWORD="))
        .map(|line| &line["WIFI_PASSWORD=".len()..])
        .unwrap_or("")
}

/// Estrae l'indirizzo IP dalla configurazione, di default 192.168.1.115
///
/// # Ritorna
/// * [u8; 4] - Indirizzo IP
pub fn get_ip_address() -> [u8; 4] {
    CONFIG
        .lines()
        .find(|line| line.starts_with("IP_ADDRESS="))
        .map(|line| {
            let ip_str = &line["IP_ADDRESS=".len()..];
            let octets: Vec<u8, 4> = ip_str
                .split(',')
                .filter_map(|s| s.trim().parse::<u8>().ok())
                .collect();
            if octets.len() == 4 {
                [octets[0], octets[1], octets[2], octets[3]]
            } else {
                [192, 168, 1, 115] // Default IP
            }
        })
        .unwrap_or([192, 168, 1, 115]) // Default IP
}

/// Estrae la submask dalla configurazione, di default 24.
///
/// # Ritorna
/// * u8 - Subnet mask
pub fn get_subnet_mask() -> u8 {
    CONFIG
        .lines()
        .find(|line| line.starts_with("SUBNET_MASK="))
        .and_then(|line| line["SUBNET_MASK=".len()..].trim().parse::<u8>().ok())
        .unwrap_or(24) // Default subnet mask
}

/// Estrae l'indirizzo del gateway dalla configurazione. Di default 192.168.1.1
///
/// # Ritorna
/// * [u8; 4] - Indirizzo IP del gateway
pub fn get_gateway_address() -> [u8; 4] {
    CONFIG
        .lines()
        .find(|line| line.starts_with("GATEWAY_ADDRESS="))
        .map(|line| {
            let ip_str = &line["GATEWAY_ADDRESS=".len()..];
            let octets: Vec<u8, 4> = ip_str
                .split(',')
                .filter_map(|s| s.trim().parse::<u8>().ok())
                .collect();
            if octets.len() == 4 {
                [octets[0], octets[1], octets[2], octets[3]]
            } else {
                [192, 168, 1, 1] // Default Gateway
            }
        })
        .unwrap_or([192, 168, 1, 1]) // Default Gateway
}

/// Genera una pagina HTML di risposta al form inviato.
///
/// # Argomenti
/// * `form` - Riferimento alla struttura FormValue con i dati del form
///
/// # Ritorna
/// * heapless::String<1024> - Pagina HTML generata
pub fn generate_html(form: &FormValue) -> heapless::String<1024> {
    let schema: heapless::String<1024> = format!(
        "{} {} {} {} {} {} {} {} {}",
        form.row_1,
        form.row_2,
        form.row_3,
        form.row_4,
        form.row_5,
        form.row_6,
        form.row_7,
        form.row_8,
        form.row_9
    )
    .unwrap_or_default();

    let mut sudoku = Sudoku::default();
    let processing = match sudoku.parse(&schema) {
        Ok(_) => match sudoku.solve_fast() {
            Ok(_) => html_table(&sudoku.grid),
            Err(e) => error_html("Error solving schema", &e),
        },
        Err(e) => error_html("Error parsing schema", &e),
    };

    form.message.borrow_mut().clear();
    form.message
        .borrow_mut()
        .push_str(&processing)
        .unwrap_or_default();
    processing
}
