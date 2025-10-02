use fixed::traits::ToFixed;
use fixed_macro::types::U56F8;
use embassy_rp::peripherals::PIO1;
use embassy_rp::pio::program::pio_asm;
use embassy_rp::pio::{Common, Config, Irq, StateMachine};
pub fn setup_pio_task_sm2<'a>(pio: &mut Common<'a, PIO1>, sm: &mut StateMachine<'a, PIO1, 2>) {
    // Setup sm2

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

#[embassy_executor::task]
pub async fn pio_task_sm2(mut irq: Irq<'static, PIO1, 3>, mut sm: StateMachine<'static, PIO1, 2>) {
    sm.set_enable(true);
    loop {
        irq.wait().await;
        log::info!("--> Solving...");
    }
}