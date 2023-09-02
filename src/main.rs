#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Pin, Speed};
use embassy_time::{Duration, Timer};
use {defmt_rtt as _, panic_probe as _};

const WORK: Duration = Duration::from_secs(15 * 60);
const NOTICE_PERIOD: Duration = Duration::from_secs(60);
const WARNING_PERIOD: Duration = Duration::from_secs(10);
const BREAK: Duration = Duration::from_secs(5 * 60);

const SHORT_BEEP: Duration = Duration::from_millis(200);
const LONG_BEEP: Duration = Duration::from_millis(600);

async fn beep<T: Pin>(speaker: &mut Output<'_, T>, duration: Duration) {
    speaker.set_high();
    Timer::after(duration).await;
    speaker.set_low();
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    info!("Hello World!");

    let mut usb_power = Output::new(p.PC13, Level::High, Speed::Low);
    let mut speaker = Output::new(p.PA6, Level::High, Speed::Low);
    let mut red = Output::new(p.PA2, Level::High, Speed::Low);
    let mut green = Output::new(p.PA1, Level::High, Speed::Low);
    let mut blue = Output::new(p.PA3, Level::High, Speed::Low);

    speaker.set_low();

    loop {
        info!("work for {}m", WORK.as_secs() / 60);
        beep(&mut speaker, SHORT_BEEP).await;
        //green led
        red.set_low();
        green.set_high();
        blue.set_low();

        usb_power.set_high();
        Timer::after(WORK - NOTICE_PERIOD).await;

        info!("sending notice {}s before break", NOTICE_PERIOD.as_secs());
        beep(&mut speaker, SHORT_BEEP).await;
        //blue led
        red.set_low();
        green.set_low();
        blue.set_high();

        Timer::after(NOTICE_PERIOD - SHORT_BEEP - WARNING_PERIOD).await;

        info!("sending warning {}s before break", WARNING_PERIOD.as_secs());
        beep(&mut speaker, LONG_BEEP).await;
        //orangeish led
        red.set_high();
        green.set_high();
        blue.set_low();

        Timer::after(WARNING_PERIOD - LONG_BEEP).await;

        info!("break for {}m", BREAK.as_secs() / 60);
        beep(&mut speaker, SHORT_BEEP).await;
        //red led
        red.set_high();
        green.set_low();
        blue.set_low();

        usb_power.set_low();
        Timer::after(BREAK).await;
    }
}
