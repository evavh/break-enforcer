#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::{
    gpio::{Level, Output, Pin, Speed},
    pwm::{
        simple_pwm::{PwmPin, SimplePwm},
        Channel, Channel1Pin,
    },
    time::hz,
};
use embassy_time::{Duration, Timer};
use {defmt_rtt as _, panic_probe as _};

const WORK_MINUTES: u64 = 60;
const WARNING_PERIOD_SECS: u64 = 10;
const BREAK_MINUTES: u64 = 10;

const BEEP_DURATION_MILLIS: u64 = 500;

// struct Led<'d, T> {
//     red: SimplePwm<'d, T>,
//     green: SimplePwm<'d, T>,
//     blue: SimplePwm<'d, T>,
// }
//
// impl<T> Led<'_, T> {
//     fn new(red: SimplePwm<T>, green: SimplePwm<T>, blue: SimplePwm<T>) -> Self {
//         Self { red, green, blue }
//     }
// }
//
// async fn beep<T: Pin>(speaker: &mut Output<'_, T>) {
//     speaker.set_high();
//     Timer::after(Duration::from_millis(BEEP_DURATION_MILLIS)).await;
//     speaker.set_low();
// }

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    info!("Hello World!");

    let mut usb_power = Output::new(p.PC13, Level::High, Speed::Low);
    //    let mut speaker = Output::new(p.PA6, Level::High, Speed::Low);
    let green_pin = PwmPin::new_ch2(p.PA1);
    let red_pin = PwmPin::new_ch3(p.PA2);
    let blue_pin = PwmPin::new_ch4(p.PA3);

    // let mut led_green =
    //     SimplePwm::new(p.TIM2, None, Some(green_pin), None, None, hz(2000));
    let mut led_red=
        SimplePwm::new(p.TIM2, None, None, Some(red_pin), None, hz(2000));
    // let mut led_blue =
    //     SimplePwm::new(p.TIM2, None, None, None, Some(blue_pin), hz(2000));

    let max_duty = led_red.get_max_duty();
//    led_green.set_duty(Channel::Ch2, max_duty / 2);
    led_red.set_duty(Channel::Ch3, max_duty / 2);
//    led_blue.set_duty(Channel::Ch4, max_duty / 2);

    loop {
        info!("high: work");
        usb_power.set_high();
        led_red.enable(Channel::Ch3);
        info!("waiting for {} minutes", WORK_MINUTES);
        //Timer::after(Duration::from_secs(WORK_MINUTES * 60 - WARNING_PERIOD_SECS)).await;
        Timer::after(Duration::from_secs(15)).await;

        info!(
            "sending warning {} seconds before break",
            WARNING_PERIOD_SECS
        );
        //beep(&mut speaker);
        //Timer::after(Duration::from_millis(WARNING_PERIOD_SECS * 1000 - BEEP_DURATION_MILLIS)).await;
        Timer::after(Duration::from_millis(0)).await;

        info!("low: break");
        usb_power.set_low();
        //Timer::after(Duration::from_secs(BREAK_MINUTES * 60)).await;
        Timer::after(Duration::from_secs(10)).await;
    }
}
