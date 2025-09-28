//! File di configurazione di varibili amnbiente
//! da modificare a seconda del proprio amnbiente.
//! TODO: cambirare ip, gateway, ssid e password.

pub const CONFIG: &str = r#"
IP_ADDRESS=192, 168, 1, 115
GATEWAY_ADDRESS=192, 168, 1, 1
WIFI_NETWORK=<your-ssid>
WIFI_PASSWORD=<your-password>
SUBNET_MASK=24
"#;
