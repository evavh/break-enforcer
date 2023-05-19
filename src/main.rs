#![no_main]
#![no_std]
#![feature(ptr_sub_ptr)]
#![feature(array_zip)]
#![feature(slice_partition_dedup)]
#![feature(asm_const)]

use defmt::info;
use defmt_rtt as _;
use fugit::RateExtU32;
use hal::{
    gpio::{self, PinExt},
    pac::Peripherals,
    prelude::_stm32f4xx_hal_gpio_GpioExt,
    rcc::RccExt,
    syscfg::SysCfgExt,
};
use stm32f4xx_hal as hal;
// global logger
use panic_probe as _;

use crate::hal::prelude::_stm32f4xx_hal_gpio_ExtiPin;

use cortex_m_rt::entry;

mod test_interrupt;

// same panicking *behavior* as `panic-probe` but doesn't print a panic message
// this prevents the panic message being printed *twice* when `defmt::panic` is invoked
#[defmt::panic_handler]
fn panic() -> ! {
    cortex_m::asm::udf()
}

/// Terminates the application and makes `probe-run` exit with exit-code = 0
pub fn exit() -> ! {
    loop {
        cortex_m::asm::bkpt();
    }
}

#[entry]
fn main() -> ! {
    let mut dp = Peripherals::take().unwrap();
    let rcc = dp.RCC.constrain();
    let clocks = rcc
        .cfgr
        .use_hse(25.MHz())
        .sysclk(84.MHz())
        // .hclk(84.MHz()) // also called AHB clock
        // // // APB2 clock; data on I/O pin is sampled into
        // // // this every APB2 clock cycle
        // .pclk1(42.MHz()) // source: https://stm32f4-discovery.net/2015/01/properly-set-clock-speed-stm32f4xx-devices/
        // .pclk2(84.MHz())
        .freeze();

    info!("{:?}", clocks);

    unsafe {
        info!(
            "plln: {:b}", // Main PLL (PLL) multiplication factor for VCO
            (*hal::pac::RCC::ptr()).pllcfgr.read().plln().bits()
        );
        info!(
            "pllp {:b}",
            (*hal::pac::RCC::ptr()).pllcfgr.read().pllp().bits()
        );
        info!(
            "pllm {:b}",
            (*hal::pac::RCC::ptr()).pllcfgr.read().pllm().bits()
        );
        info!(
            "pllq {:b}",
            (*hal::pac::RCC::ptr()).pllcfgr.read().pllq().bits()
        );
        info!(
            "pllsrc is hse {}",
            (*hal::pac::RCC::ptr()).pllcfgr.read().pllsrc().is_hse()
        );

        info!(
            "PLL on: {}",
            (*hal::pac::RCC::ptr()).cr.read().pllon().is_on()
        );
        info!(
            "system clock is pll: {}",
            (*hal::pac::RCC::ptr()).cfgr.read().sw().is_pll()
        );
        info!(
            "AHB prescaler hclock {:b}",
            (*hal::pac::RCC::ptr()).cfgr.read().hpre().bits()
        );
    }

    // set power on usb (prototype needs this)
    let gpio_c = dp.GPIOC.split();
    let mut usb_enable = gpio_c.pc13.into_push_pull_output();
    usb_enable.set_high();

    // set debug pin (pa0) to fast output
    let gpio_a = dp.GPIOA.split();
    let mut debug_pin = gpio_a.pa0.into_push_pull_output();
    debug_pin.set_speed(gpio::Speed::VeryHigh);
    debug_pin.set_low();

    // this loop gets us 12-24 MHZ (mostly 12) which is lower
    // then expected.....
    // since between each store (=toggle) there are 2 cycles
    // 24 MHZ means the clock runs at 48 MHZ
    // it does:
    //   str r4 [r0] // 2 cycles
    //   str r1 [r0] // 2 cycles
    //   repeat ..
    // loop {
    //     debug_pin.set_high();
    //     debug_pin.set_low();
    // }

    unsafe {
        // output the high speed external clock on PA8
        // 3 MHZ at div5 = 15
        // 4 MHZ at div4 = 16
        // 4.8 Mhz at div3 = 15-ish
        // 8 Mhz at div 2 = 16
        //
        // PLL
        // 3 MHZ at div 5 = 15
        // 4 MHZ at div 4 = 16
        // 6 MHZ at div 3 = 18
        // 8 MHZ at div 2 = 16
        // 8 MHZ at div 1 = 16
        (*hal::pac::RCC::ptr()).cfgr.write(|w| w.mco1().hse());
        (*hal::pac::RCC::ptr()).cfgr.write(|w| w.mco1pre().div4());
    }
    // set clock out pin to alternate function 0
    let _ = gpio_a.pa8.into_alternate::<0>();

    let gpio_b = dp.GPIOB.split();
    let mut usb = gpio_b.pb1.into_floating_input();
    let usb_pin = usb.pin_id();

    info!("usb pin: pb{}", usb_pin);
    // get adress of GPIOB's IDR (input data) register. Accessed as 32 bit
    // word, however only the lower 16 bit represent pin values
    let usb_data_plus = unsafe { (*hal::pac::GPIOB::ptr()).idr.as_ptr() };
    info!(
        "usb data+ (pb{}) addr (PB in): {:x}",
        usb_pin, usb_data_plus
    );
    let debug_out = unsafe { (*hal::pac::GPIOC::ptr()).odr.as_ptr() };
    info!("debug out (pa0) addr (PC out): {:x}", debug_out);

    // exit();

    let mut syscfg = dp.SYSCFG.constrain();
    usb.make_interrupt_source(&mut syscfg);
    usb.enable_interrupt(&mut dp.EXTI);
    usb.trigger_on_edge(&mut dp.EXTI, gpio::Edge::Falling);
    let interrupt_number = usb.interrupt();

    // clear pending interrupts on usb gpio
    cortex_m::peripheral::NVIC::unpend(interrupt_number);
    unsafe {
        // enable interrupt on usb gpio
        cortex_m::peripheral::NVIC::unmask(interrupt_number);
    }

    loop {}
}
