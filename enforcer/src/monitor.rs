use defmt::println;
use embassy_stm32::exti::{Channel, ExtiInput};
use embassy_stm32::gpio::{self, Input, Pull};
use embassy_stm32::Peripheral;
use embassy_time::{Duration, Instant};

pub struct UsbData {
    channel_a: ExtiInput<'static, gpio::AnyPin>,
    channel_b: ExtiInput<'static, gpio::AnyPin>,
}
impl UsbData {
    pub(crate) fn new<CA, PA, CB, PB>(pin_a: PA, ch_a: CA, pin_b: PB, ch_b: CB) -> Self
    where
        PA: gpio::Pin,
        PB: gpio::Pin,
        CA: Peripheral<P = PA::ExtiChannel> + Channel,
        CB: Peripheral<P = PB::ExtiChannel> + Channel,
    {
        let input_a = Input::new(pin_a.degrade(), Pull::None);
        let input_b = Input::new(pin_b.degrade(), Pull::None);
        Self {
            channel_a: ExtiInput::new(input_a, ch_a.degrade()),
            channel_b: ExtiInput::new(input_b, ch_b.degrade()),
        }
    }
}

#[embassy_executor::task]
pub async fn task(mut usb: UsbData) {
    let mut edges_per_sec = 0usize;
    let mut last_print = Instant::now();
    loop {
        usb.channel_a.wait_for_rising_edge().await;
        edges_per_sec += 1;
        if last_print.elapsed() >= Duration::from_secs(1) {
            last_print = Instant::now();
            println!("rising edges detected per seconds: {}", edges_per_sec);
            edges_per_sec = 0;
        }
    }
}
