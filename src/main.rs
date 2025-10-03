//! # Raspberry Pico 2 W Sudoku Web Server
//!
//! This application implements a web server for Raspberry Pico 2 W that provides:
//! - HTML form interface for 9x9 Sudoku puzzle input
//! - HTTP POST endpoint for puzzle submission using the `picoserve` crate
//! - Serial communication via UART (GP0 and GP1)
//! - CYW43 WiFi chip LED control
//! - Static IP address configuration
//! - High-precision PIO timer demonstration
//!
//! # Requirements
//!
//! This project requires Rust nightly toolchain (see README for configuration details).

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
use core::cell::RefCell;
use critical_section::Mutex as CsMutex;
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

/// Wrapper for sharing CYW43 control across Embassy tasks.
///
/// This structure provides a thread-safe way to share the WiFi control interface
/// between multiple asynchronous tasks using a static mutex.
#[derive(Clone, Copy)]
pub struct SharedControl(&'static Mutex<CriticalSectionRawMutex, Control<'static>>);

/// Type alias for PIO1 State Machine 2.
type Sm2StateMachine = embassy_rp::pio::StateMachine<'static, PIO1, 2>;

/// Type alias for the mutex protecting SM2.
type Sm2Mutex = Mutex<CriticalSectionRawMutex, Sm2StateMachine>;

/// Type alias for the thread-safe cell containing optional SM2 reference.
type Sm2Cell = CsMutex<RefCell<Option<&'static Sm2Mutex>>>;

/// Wrapper for sharing PIO State Machine 2 across Embassy tasks.
///
/// This structure provides a thread-safe way to share the PIO state machine
/// between multiple asynchronous tasks, primarily used during HTML generation
/// for high-precision timing measurements.
#[derive(Clone, Copy)]
pub struct SharedSm2(&'static Sm2Mutex);

/// Global static storage for SharedSm2.
///
/// Thread-safe implementation using critical_section::Mutex + RefCell.
/// This avoids `static mut` and unsafe code while maintaining embedded safety.
static SHARED_SM2_CELL: Sm2Cell = CsMutex::new(RefCell::new(None));

/// Retrieves the global SharedSm2 reference in a thread-safe manner.
///
/// # Returns
///
/// * `Some(SharedSm2)` - If the state machine has been initialized
/// * `None` - If the state machine has not been initialized yet
///
/// # Thread Safety
///
/// This function uses critical sections to ensure safe concurrent access.
pub fn get_shared_sm2() -> Option<SharedSm2> {
    critical_section::with(|cs| {
        SHARED_SM2_CELL.borrow(cs).borrow().as_ref().map(|ptr| SharedSm2(ptr))
    })
}

/// Initializes the global SharedSm2 reference.
///
/// # Arguments
///
/// * `sm2` - Static reference to the PIO state machine mutex
///
/// # Panics
///
/// Panics if called more than once. This function should only be called
/// from `main` during initialization.
///
/// # Thread Safety
///
/// Uses critical sections to ensure safe initialization.
fn set_shared_sm2(sm2: &'static Sm2Mutex) {
    critical_section::with(|cs| {
        let mut cell = SHARED_SM2_CELL.borrow(cs).borrow_mut();
        if cell.is_some() {
            log::error!("SHARED_SM2 already initialized - set_shared_sm2() called multiple times");
            core::panic!("SHARED_SM2 already initialized - set_shared_sm2() called multiple times");
        }
        *cell = Some(sm2);
    });
}

/// Main entry point for the Embassy executor.
///
/// Initializes all hardware peripherals, WiFi connection, network stack,
/// and spawns all background tasks including:
/// - USB logger
/// - WiFi driver (CYW43)
/// - Network stack
/// - UART reader
/// - LED blinker
/// - Periodic timer
/// - Web server task pool
///
/// # Arguments
///
/// * `spawner` - Embassy task spawner for launching concurrent tasks
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Start USB logger
    let driver = Driver::new(p.USB, UsbIrqs);
    spawner.must_spawn(logger_task(driver));
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

    // Start WiFi chip management task
    spawner.must_spawn(cyw43_task(runner));
    panic_led_loop!(control);

    // Initialize PIO1 for high-precision timer demonstration
    // The timer generates interrupts handled entirely by PIO without CPU involvement
    let pio1 = p.PIO1;
    let Pio {
        mut common,
        mut sm2,
        irq3,
        ..
    } = Pio::new(pio1, IrqPIO1);

    pio::setup_pio_task_sm2(&mut common, &mut sm2);

    // Initialize global static SM2 reference
    // SM2 is activated only during HTML generation for timing measurements
    let sm2_ref = make_static!(Sm2Mutex, Mutex::new(sm2));
    set_shared_sm2(sm2_ref);

    // Start PIO task to handle SM2 interrupts (SM2 enabled/disabled by Sm2Guard during HTML generation)
    let shared_sm2 = SharedSm2(sm2_ref);
    spawner.must_spawn(pio::pio_task_sm2(irq3, shared_sm2));
    panic_led_loop!(control);

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let uart_tx: UartTx<'_, Async> = UartTx::new(p.UART0, p.PIN_0, p.DMA_CH1, Config::default());
    let uart_rx = UartRx::new(p.UART1, p.PIN_5, UartIrqs, p.DMA_CH2, Config::default());

    // Start UART reader task
    spawner.must_spawn(reader(uart_rx));
    panic_led_loop!(control);

    // Generate random seed for network stack
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

    // Start network stack management task
    spawner.must_spawn(net_task(runner));
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
    // Retrieve IPv4 configuration
    if let Some(config) = stack.config_v4() {
        let ip = config.address.address();
        log::info!("Assigned IP: {ip}");
    }

    // Create shared control wrapper for task sharing
    let shared_control = SharedControl(
        make_static!(Mutex<CriticalSectionRawMutex, Control<'static>>, Mutex::new(control)),
    );

    // Start LED blinker task for CYW43 chip
    spawner.must_spawn(blink_task_shared(shared_control, uart_tx));
    panic_led_loop_shared!(shared_control);

    // Start ticker task to demonstrate system is alive
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

    // Start web server task pool to handle parallel HTTP requests
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

// Background tasks

/// WiFi driver task for CYW43 chip.
///
/// Runs the WiFi driver event loop indefinitely, handling low-level
/// WiFi operations including packet transmission/reception and chip management.
///
/// # Arguments
///
/// * `runner` - CYW43 WiFi driver runner instance
///
/// # Returns
///
/// Never returns (infinite loop)
#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

/// Network stack task.
///
/// Runs the Embassy network stack event loop indefinitely, handling
/// TCP/IP protocol operations, socket management, and network state.
///
/// # Arguments
///
/// * `runner` - Embassy network stack runner instance
///
/// # Returns
///
/// Never returns (infinite loop)
#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

/// USB logger task.
///
/// Provides logging output over USB serial connection with 1024 byte buffer
/// and Info level filtering.
///
/// # Arguments
///
/// * `driver` - USB driver instance for serial communication
#[embassy_executor::task]
async fn logger_task(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

/// Periodic timer task.
///
/// Logs a random number every 60 seconds to demonstrate that the system
/// is alive and responsive.
///
/// # Returns
///
/// Never returns (infinite loop)
#[embassy_executor::task]
async fn ticker_task() {
    let mut ticker = Ticker::every(Duration::from_secs(ELAPSED_SECS));
    loop {
        ticker.next().await;
        // Generate random number for logging
        let random_number: u32 = RoscRng.next_u32();

        log::info!("[Timer tick {random_number:2}]");
    }
}

/// UART receiver task.
///
/// Continuously reads 32-byte chunks from the UART interface and logs
/// the received data. Used for debugging and external communication.
///
/// # Arguments
///
/// * `rx` - UART receiver instance configured for asynchronous operation
///
/// # Returns
///
/// Never returns (infinite loop)
#[embassy_executor::task]
async fn reader(mut rx: UartRx<'static, Async>) {
    info!("Reading...");
    loop {
        // read a total of 4 transmissions (32 / 8) and then print the result
        let mut buf = [0; 32];
        rx.read(&mut buf).await.unwrap();
        info!("RX {:?}", buf);
    }
}

/// LED blinker task with UART status output.
///
/// Toggles the WiFi chip's LED every 250ms and sends status messages
/// via UART to indicate the LED state.
///
/// # Arguments
///
/// * `shared_control` - Shared WiFi controller for LED access
/// * `uart_tx` - UART transmitter for sending status messages
///
/// # Returns
///
/// Never returns (infinite loop)
#[embassy_executor::task]
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

/// Web server task pool handler.
///
/// Handles HTTP requests on port 80 using the picoserve framework.
/// Multiple instances run concurrently (defined by WEB_TASK_POOL_SIZE)
/// to handle parallel client connections.
///
/// Each task maintains its own TCP and HTTP buffers for request processing.
///
/// # Arguments
///
/// * `id` - Unique identifier for this task instance
/// * `stack` - Network stack for TCP/IP operations
/// * `app` - Application router defining HTTP endpoints
/// * `config` - Server configuration including timeouts and keep-alive settings
/// * `state` - Shared application state (includes WiFi control)
///
/// # Returns
///
/// Never returns (infinite loop)
#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
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
