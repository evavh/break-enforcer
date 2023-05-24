#![no_main]
#![no_std]

use core::arch::asm;

use defmt::info;
use defmt_rtt as _;
use fugit::RateExtU32;
use hal::{gpio, pac::Peripherals, prelude::_stm32f4xx_hal_gpio_GpioExt, rcc::RccExt};
use stm32f4xx_hal as hal;
// global logger
use panic_probe as _;

use cortex_m_rt::entry;

#[entry]
fn main() -> ! {
    let dp = Peripherals::take().unwrap();
    let rcc = dp.RCC.constrain();
    let _clocks = rcc.cfgr.use_hse(25.MHz()).sysclk(84.MHz()).freeze();

    // // Setup handler for device peripherals
    // let dp = unsafe { hal::pac::Peripherals::steal() };
    //
    // // Enable HSE Clock
    // dp.RCC.cr.write(|w| w.hseon().set_bit());
    //
    // // Wait for HSE clock to become ready
    // while dp.RCC.cr.read().hserdy().is_not_ready() {}
    //
    // // enable instruction cache, prefetch buffer
    // // data cache and set flash latency to 5 wait states
    // dp.FLASH.acr.write(|w| {
    //     w.icen().set_bit();
    //     w.prften().set_bit();
    //     w.dcen().set_bit();
    //     w.latency().ws5()
    // });
    //
    // // Configure bus prescalars
    // unsafe {
    //     dp.RCC.cfgr.write(|w| {
    //         w.hpre().div1(); // AHB prescalar
    //         w.ppre1().bits(2);
    //         w.ppre2().bits(1)
    //     })
    // }
    //
    // const PLLM: u8 = 25;
    // const PLLN: u16 = 168;
    // const PLLP_DIV_2: u8 = 0;
    // unsafe {
    //     dp.RCC.pllcfgr.write(|w| {
    //         w.pllsrc().hse();
    //         w.pllm().bits(PLLM);
    //         w.plln().bits(PLLN);
    //         w.pllp().bits(PLLP_DIV_2)
    //     });
    // }
    //
    // // turn on pll
    // dp.RCC.cr.write(|w| {
    //     w.hseon().on();
    //     w.pllon().on()
    // });
    //
    // info!("waiting for PLL to become ready");
    // while dp.RCC.cr.read().pllrdy().is_not_ready() {}
    //
    // // Select PLL as System Clock Source
    // unsafe {
    //     dp.RCC.cfgr.write(|w| {
    //         w.sw().pll();
    //         w.hpre().div1(); // AHB prescalar
    //         w.ppre1().bits(2);
    //         w.ppre2().bits(1)
    //     });
    // }
    //
    // info!("waiting for PLL to be selected as System Clock Source");
    // while !dp.RCC.cfgr.read().sws().is_pll() {}

    info!("done with clock setup");
    // set debug pin (pa0) to fast output
    let gpio_a = dp.GPIOA.split();
    let mut debug_pin = gpio_a.pa0.into_push_pull_output();
    debug_pin.set_speed(gpio::Speed::VeryHigh);
    debug_pin.set_low();

    unsafe {
        // output pll clock = sysclock here divided by 5 on pa8
        (*hal::pac::RCC::ptr()).cfgr.modify(|_, w| {
            w.mco1pre().div5();
            w.mco1().pll()
        });
    }
    // set clock out pin to alternate function 0
    let _ = gpio_a.pa8.into_alternate::<0>();

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
