//! # Utility Functions
//!
//! Utility functions for HTML page generation and configuration parsing.
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

/// Generates an HTML error page.
///
/// # Arguments
///
/// * `msg` - Error message to display
/// * `err` - Error object implementing Debug trait
///
/// # Returns
///
/// `heapless::String<1024>` containing the generated HTML error page
pub fn error_html<T: core::fmt::Debug>(msg: &str, err: &T) -> heapless::String<1024> {
    format!(
        "{header}<h1>{msg}: {:?}</h1>{footer}",
        err,
        header = HTML_HEADER,
        footer = HTML_FOOTER
    )
    .unwrap_or_default()
}

/// Generates an HTML table from a solved Sudoku grid.
///
/// # Arguments
///
/// * `grid` - Reference to the 9x9 solved Sudoku matrix
///
/// # Returns
///
/// `heapless::String<1024>` containing the generated HTML table
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

/// Extracts the WiFi SSID from the configuration.
///
/// # Returns
///
/// WiFi network SSID as a static string slice
pub fn get_ssid() -> &'static str {
    CONFIG
        .lines()
        .find(|line| line.starts_with("WIFI_NETWORK="))
        .map(|line| &line["WIFI_NETWORK=".len()..])
        .unwrap_or("")
}

/// Extracts the WiFi password from the configuration.
///
/// # Returns
///
/// WiFi network password as a static string slice
pub fn get_wifi_password() -> &'static str {
    CONFIG
        .lines()
        .find(|line| line.starts_with("WIFI_PASSWORD="))
        .map(|line| &line["WIFI_PASSWORD=".len()..])
        .unwrap_or("")
}

/// Extracts the IP address from the configuration.
///
/// # Returns
///
/// IP address as a 4-byte array. Defaults to `[192, 168, 1, 115]` if not configured
/// or if parsing fails.
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

/// Extracts the subnet mask from the configuration.
///
/// # Returns
///
/// Subnet mask as CIDR prefix length (e.g., 24 for 255.255.255.0).
/// Defaults to `24` if not configured or if parsing fails.
pub fn get_subnet_mask() -> u8 {
    CONFIG
        .lines()
        .find(|line| line.starts_with("SUBNET_MASK="))
        .and_then(|line| line["SUBNET_MASK=".len()..].trim().parse::<u8>().ok())
        .unwrap_or(24) // Default subnet mask
}

/// Extracts the gateway address from the configuration.
///
/// # Returns
///
/// Gateway IP address as a 4-byte array. Defaults to `[192, 168, 1, 1]` if not
/// configured or if parsing fails.
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

/// Generates an HTML response page from submitted form data.
///
/// Parses the Sudoku puzzle from the form, attempts to solve it, and generates
/// an HTML table with the solution or an error message.
///
/// # Arguments
///
/// * `form` - Reference to the FormValue structure containing form data
///
/// # Returns
///
/// `heapless::String<1024>` containing the generated HTML response page
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
