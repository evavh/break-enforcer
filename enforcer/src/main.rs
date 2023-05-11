#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use core::mem;

use cortex_m::peripheral::NVIC;
use cortex_m_rt::entry;
use defmt::*;
use embassy_stm32::executor::{Executor, InterruptExecutor};
use embassy_stm32::interrupt;
use embassy_stm32::pac::Interrupt;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

mod control;
mod manage;
mod monitor;

static EXECUTOR_HIGH: InterruptExecutor = InterruptExecutor::new();
static EXECUTOR_LOW: StaticCell<Executor> = StaticCell::new();

/// Note DO NOT USE timer 5 for other interrupts
#[interrupt]
unsafe fn TIM5() {
    EXECUTOR_HIGH.on_interrupt()
}

#[entry]
fn main() -> ! {
    let p = embassy_stm32::init(Default::default());
    drop(p.EXTI0); // Not availible as its used for schedualling high priority executor

    let usb_monitor = monitor::UsbData::new(p.PB1, p.EXTI1, p.PB2, p.EXTI2);
    let usb_power = control::UsbPower::new(p.PC13, p.PC14);

    info!("Hello World!");

    let mut nvic: NVIC = unsafe { mem::transmute(()) };

    // High-priority executor: SWI1_EGU1, priority level 6
    unsafe { nvic.set_priority(Interrupt::EXTI0, 6 << 5) };
    let spawner = EXECUTOR_HIGH.start(Interrupt::TIM5);
    unwrap!(spawner.spawn(monitor::task(usb_monitor)));

    // Low priority executor: runs in thread mode, using WFE/SEV
    let executor = EXECUTOR_LOW.init(Executor::new());
    executor.run(|spawner| {
        unwrap!(spawner.spawn(manage::manage(usb_power)));
    });
}
