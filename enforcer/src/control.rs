use embassy_stm32::gpio::{self, Output, Speed, Level};

pub struct UsbPower {
    output_a: Output<'static, gpio::AnyPin>,
    output_b: Output<'static, gpio::AnyPin>,
}

impl UsbPower {
    pub(crate) fn new<PA, PB>(pin_a: PA, pin_b: PB) -> Self
    where
        PA: gpio::Pin,
        PB: gpio::Pin,
    {
        let pin_a = pin_a.degrade();
        let pin_b = pin_b.degrade();
        Self {
        output_a : Output::new(pin_a, Level::High, Speed::Low),
        output_b : Output::new(pin_b, Level::High, Speed::Low),
        }

    }
}
