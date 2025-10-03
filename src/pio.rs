//! # PIO State Machine Configuration
//!
//! This module configures and manages PIO (Programmable I/O) state machine 2
//! for high-precision timing and interrupt generation.

use embassy_rp::peripherals::PIO1;
use embassy_rp::pio::program::pio_asm;
use embassy_rp::pio::{Common, Config, Irq, StateMachine};
use fixed::traits::ToFixed;
use fixed_macro::types::U56F8;

/// Sets up PIO State Machine 2 for periodic interrupt generation.
///
/// Configures SM2 to repeatedly trigger IRQ 3 at approximately 2kHz.
/// The state machine uses a delay loop to create precise timing without
/// CPU intervention.
///
/// # Arguments
///
/// * `pio` - Mutable reference to the PIO1 common resources
/// * `sm` - Mutable reference to State Machine 2
///
/// # PIO Program
///
/// The loaded program:
/// - Sets counter to 10
/// - Decrements counter with delays
/// - Triggers IRQ 3 when counter reaches 0
/// - Repeats indefinitely
pub fn setup_pio_task_sm2<'a>(pio: &mut Common<'a, PIO1>, sm: &mut StateMachine<'a, PIO1, 2>) {
    // Repeatedly trigger IRQ 3
    let prg = pio_asm!(
        ".origin 0",
        ".wrap_target",
        "set x,10",
        "delay:",
        "jmp x-- delay [15]",
        "irq 3 [15]",
        ".wrap",
    );
    let mut cfg = Config::default();
    cfg.use_program(&pio.load_program(&prg.program), &[]);
    cfg.clock_divider = (U56F8!(125_000_000) / 2000).to_fixed();
    sm.set_config(&cfg);
}

/// PIO State Machine 2 interrupt handler task.
///
/// This task waits for IRQ 3 from SM2 and logs when interrupts occur.
/// SM2 is enabled/disabled dynamically by `Sm2Guard` during HTML generation.
///
/// # Arguments
///
/// * `irq` - IRQ 3 handler for PIO1
/// * `_shared_sm2` - Shared reference to State Machine 2 (unused, SM2 controlled by Sm2Guard)
///
/// # Behavior
///
/// - Waits for IRQ 3 events (which occur only when SM2 is enabled)
/// - Logs each interrupt occurrence during Sudoku solving
/// - Runs indefinitely
#[embassy_executor::task]
pub async fn pio_task_sm2(mut irq: Irq<'static, PIO1, 3>, _shared_sm2: crate::SharedSm2) {
    loop {
        irq.wait().await;
        log::info!("--> Solving...");
    }
}
