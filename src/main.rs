#![no_main]
#![no_std]

use core::arch::asm;
use core::panic::PanicInfo;

// use defmt_rtt as _;
use fugit::RateExtU32;
use hal::{gpio, pac::Peripherals, prelude::_stm32f4xx_hal_gpio_GpioExt, rcc::RccExt};
use stm32f4xx_hal as hal;
// global logger
// use panic_probe as _;

use cortex_m_rt::entry;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[entry]
fn main() -> ! {
    let dp = Peripherals::take().unwrap();
    let rcc = dp.RCC.constrain();
    let _clocks = rcc.cfgr.use_hse(25.MHz()).sysclk(84.MHz()).freeze();

    // set debug pin (pa0) to fast output
    let gpio_a = dp.GPIOA.split();
    let mut debug_pin = gpio_a.pa0.into_push_pull_output();
    debug_pin.set_speed(gpio::Speed::VeryHigh);
    debug_pin.set_low();

    unsafe {
        // output pll clock = sysclock here divided by 5 on pa8
        (*hal::pac::RCC::ptr()).cfgr.write(|w| {
            w.mco1().pll();
            w.mco1pre().div5()
        });
    }
    // set clock out pin to alternate function 0
    let _ = gpio_a.pa8.into_alternate::<0>();

    // at 84 Mhz 1 cycle ~= 12 nano sec
    // 20 uS for 100 cycles
    // -> 200 nano sec for 1 cycle
    loop {
        debug_pin.set_high();
        unsafe {
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
            asm!("NOP");
        }
        debug_pin.set_low();
    }
}
