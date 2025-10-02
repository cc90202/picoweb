//! Questa applicazione per Raspberry Pico 2 W
//! realizza un web server che espone un servizio
//! per l'inserimento di un form HTML per uno schema
//! di Sudoku 9x9. Lo schema viene inviato via HTTP POST
//! attraverso il crate 'picoserve'.
//! Viene anche gestita la comunicazione seriale via UART su GP0 e GP1.
//! Viene anche gestito il LED collegato al chip WiFi CYW43.
//! Viene infine assegnato un indirizzo IP statico.
//! Viene impostata la modalità di compilazione con nightly (vedi README).

#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

mod configuration;
mod sudoku;
#[macro_use]
mod utility;
mod app_props;
mod app_state;
mod form_value;
mod pio;

use crate::app_props::AppProps;
use crate::app_state::AppState;
use cyw43::{Control, JoinOptions};
use cyw43_pio::{PioSpi, RM2_CLOCK_DIVIDER};
use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_net::Ipv4Address;
use embassy_rp::bind_interrupts;
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0, PIO1, UART1, USB};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::uart::{Async, Config, InterruptHandler as UartInterruptHandler, UartRx, UartTx};
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker, Timer};
use panic_persist as _;
use picoserve::{AppRouter, AppWithStateBuilder, make_static};
use static_cell::StaticCell;
use utility::*;

const WEB_TASK_POOL_SIZE: usize = 10;
const ELAPSED_SECS: u64 = 60;

// Program metadata for `picotool info`.
// This isn't needed, but it's recommended to have these minimal entries.
#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Blinky Example"),
    embassy_rp::binary_info::rp_program_description!(
        c"Questo programma gira su Pico 2 W e risolve uno schema di Sudoku 9x9."
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

// Interrupt handlers
bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

bind_interrupts!(struct UartIrqs {
    UART1_IRQ => UartInterruptHandler<UART1>;
});

bind_interrupts!(struct UsbIrqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
});

bind_interrupts!(struct IrqPIO1 {
    PIO1_IRQ_0 => InterruptHandler<PIO1>;
});

/// Struttura per condividere il controller tra task embassy diversi
#[derive(Clone, Copy)]
struct SharedControl(&'static Mutex<CriticalSectionRawMutex, Control<'static>>);

/// Entry point principale secondo Embassy
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Parte il logger su USB
    let driver = Driver::new(p.USB, UsbIrqs);
    spawner.must_spawn(logger_task(driver)); //<---- 1
    if let Some(panic_message) = panic_persist::get_panic_message_utf8() {
        log::error!("{panic_message}");
        loop {
            embassy_time::Timer::after_secs(5).await;
        }
    }

    // Firmware files for the CYW43xxx WiFi chip.
    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

    // To make flashing faster for development, you may want to flash the firmwares independently
    // at hardcoded addresses, instead of baking them into the program with `include_bytes!`:
    //     probe-rs download ../../cyw43-firmware/43439A0.bin --binary-format bin --chip RP235x --base-address 0x10100000
    //     probe-rs download ../../cyw43-firmware/43439A0_clm.bin --binary-format bin --chip RP235x --base-address 0x10140000
    //let fw = unsafe { core::slice::from_raw_parts(0x10100000 as *const u8, 230321) };
    //let clm = unsafe { core::slice::from_raw_parts(0x10140000 as *const u8, 4752) };

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs); // <---- PIO0 for SPI communication
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        // SPI communication won't work if the speed is too high, so we use a divider larger than `DEFAULT_CLOCK_DIVIDER`.
        // See: https://github.com/embassy-rs/embassy/issues/3960.
        RM2_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        p.DMA_CH0,
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;

    // parte il task di gestione del chip WiFi
    spawner.must_spawn(cyw43_task(runner)); //<---- 2
    panic_led_loop!(control);

    // PIO1 per un timer di esempio ad altissima precisione che genera un interrupt
    // e viene gestito dal PIO senza passare da CPU.
    let pio1 = p.PIO1;
    let Pio {
        // destrutturazione per prendere solo quello che serve
        mut common,
        irq3,
        mut sm2,
        ..
    } = Pio::new(pio1, IrqPIO1);

    pio::setup_pio_task_sm2(&mut common, &mut sm2);

    spawner.must_spawn(pio::pio_task_sm2(irq3, sm2)); //<---- esempio di task con PIO1 e interrupt
    panic_led_loop!(control);

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let uart_tx: UartTx<'_, Async> = UartTx::new(p.UART0, p.PIN_0, p.DMA_CH1, Config::default());
    let uart_rx = UartRx::new(p.UART1, p.PIN_5, UartIrqs, p.DMA_CH2, Config::default());

    // Fa partire la UART (lettura)
    spawner.must_spawn(reader(uart_rx)); //<---- 3
    panic_led_loop!(control);

    // Genera un random seed per il network stack
    let seed: u64 = RoscRng.next_u64();
    log::info!("Random seed value seeded to {}", seed);

    let ip = get_ip_address();
    log::info!("IP address: {:?}", ip);
    let gateway = get_gateway_address();
    let (stack, runner) = embassy_net::new(
        net_device,
        embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
            address: embassy_net::Ipv4Cidr::new(
                Ipv4Address::new(ip[0], ip[1], ip[2], ip[3]),
                get_subnet_mask(),
            ),
            gateway: Some(Ipv4Address::new(
                gateway[0], gateway[1], gateway[2], gateway[3],
            )),
            dns_servers: Default::default(),
        }),
        make_static!(
            embassy_net::StackResources::<WEB_TASK_POOL_SIZE>,
            embassy_net::StackResources::new()
        ),
        seed,
    );

    // parte il task di gestione del network
    spawner.must_spawn(net_task(runner)); //<---- 4
    panic_led_loop!(control);

    while let Err(err) = control
        .join(get_ssid(), JoinOptions::new(get_wifi_password().as_bytes()))
        .await
    {
        log::info!("join failed with status={}", err.status);
        control.gpio_set(0, true).await;
    }

    log::info!("waiting for DHCP...");
    stack.wait_config_up().await;

    // And now we can use it!
    log::info!("Stack is up!");
    // Recupera la configurazione IPv4
    if let Some(config) = stack.config_v4() {
        let ip = config.address.address();
        log::info!("Assigned IP: {ip}");
    }

    // Definiamo un controllore comune da condividere tra i task
    let shared_control = SharedControl(
        make_static!(Mutex<CriticalSectionRawMutex, Control<'static>>, Mutex::new(control)),
    );

    // Fa partire il blink del LED collegato al cyw43
    spawner.must_spawn(blink_task_shared(shared_control, uart_tx)); //<---- 5
    panic_led_loop_shared!(shared_control);

    // Fa partire un timer: per ora non serve a molto, se non a dimostrare
    // che il sistema è vivo.
    spawner.must_spawn(ticker_task());
    panic_led_loop_shared!(shared_control);

    let app = make_static!(AppRouter<AppProps>, AppProps.build_app());

    let config2 = make_static!(
        picoserve::Config::<Duration>,
        picoserve::Config::new(picoserve::Timeouts {
            start_read_request: Some(Duration::from_secs(5)),
            persistent_start_read_request: Some(Duration::from_secs(1)),
            read_request: Some(Duration::from_secs(1)),
            write: Some(Duration::from_secs(1)),
        })
        .keep_connection_alive()
    );

    // Fa partire i task del web server per rispondere a diverse richieste in parallelo,
    for id in 0..WEB_TASK_POOL_SIZE - 2 {
        unwrap!(spawner.spawn(web_task(
            id,
            stack,
            app,
            config2,
            AppState { shared_control },
        )));
    }

    log::info!(
        "Web Server running on http://{}/",
        stack.config_v4().unwrap().address.address()
    );
}

// Tasks that run in the background:
/// WIFI task runner
///
/// # Argomenti
/// * `runner` - cyw43 runner
///
/// # Ritorna
/// * ! - Non ritorna mai
#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
/// Network task runner
///
/// # Argomenti
/// * `runner` - embassy net runner
///
/// # Ritorna
/// * ! - Non ritorna mai
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
/// Logger task for USB
///
/// # Argomenti
/// * `driver` - USB driver
///
/// # Ritorna
/// * ! - Non ritorna mai
async fn logger_task(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::task]
/// Timer task che logga un valore random ogni 5 secondi
///
/// # Ritorna
/// * ! - Non ritorna mai
async fn ticker_task() {
    let mut ticker = Ticker::every(Duration::from_secs(ELAPSED_SECS));
    loop {
        ticker.next().await;
        // calcola un random number
        let random_number: u32 = RoscRng.next_u32();

        log::info!("[Timer tick {random_number:2}]");
    }
}

#[embassy_executor::task]
/// UART reader task
///
/// # Argomenti
/// * `rx` - UART receiver
///
/// # Ritorna
/// * ! - Non ritorna mai
async fn reader(mut rx: UartRx<'static, Async>) {
    info!("Reading...");
    loop {
        // read a total of 4 transmissions (32 / 8) and then print the result
        let mut buf = [0; 32];
        rx.read(&mut buf).await.unwrap();
        info!("RX {:?}", buf);
    }
}

#[embassy_executor::task]
/// Blink task che toggla il LED collegato al chip WiFi CYW43
///
/// # Argomenti
/// * `shared_control` - Controller condiviso per il WiFi
/// * `uart_tx` - UART transmitter
///
/// # Ritorna
/// * ! - Non ritorna mai
async fn blink_task_shared(shared_control: SharedControl, mut uart_tx: UartTx<'static, Async>) {
    let delay = Duration::from_millis(250);
    loop {
        let msg = "Led on!\r\n".as_bytes();
        shared_control.0.lock().await.gpio_set(0, true).await;
        uart_tx.write(msg).await.unwrap();
        Timer::after(delay).await;

        let msg = "Led off!\r\n".as_bytes();
        shared_control.0.lock().await.gpio_set(0, false).await;
        uart_tx.write(msg).await.unwrap();
        Timer::after(delay).await;
    }
}

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
/// Web server task che risponde alle richieste HTTP
///
/// # Argomenti
///
/// * `id` - ID del task
/// * `stack` - Stack di rete
/// * `app` - Router dell'applicazione
/// * `config` - Configurazione del server
/// * `state` - Stato dell'applicazione
async fn web_task(
    id: usize,
    stack: embassy_net::Stack<'static>,
    app: &'static AppRouter<AppProps>,
    config: &'static picoserve::Config<Duration>,
    state: AppState,
) -> ! {
    let port = 80;
    let mut tcp_rx_buffer = [0; 1024];
    let mut tcp_tx_buffer = [0; 1024];
    let mut http_buffer = [0; 2048];

    picoserve::listen_and_serve_with_state(
        id,
        app,
        config,
        stack,
        port,
        &mut tcp_rx_buffer,
        &mut tcp_tx_buffer,
        &mut http_buffer,
        &state,
    )
    .await
}
