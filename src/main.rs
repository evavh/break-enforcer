#![no_main]
#![no_std]
#![feature(ptr_sub_ptr)]
#![feature(array_zip)]
#![feature(slice_partition_dedup)]
#![feature(asm_const)]

use cortex_m::asm::delay;
use defmt::info;
use defmt_rtt as _;
// use fugit::RateExtU32;
use hal::pac;
use stm32f4xx_hal as hal;
// global logger
use panic_probe as _;

// use cortex_m_rt::entry;

mod test_interrupt;

// Imports
use cortex_m_rt::entry;

// based on: https://controllerstech.com/stm32-clock-setup-using-registers/
// https://kleinembedded.com/stm32-without-cubeide-part-2-cmsis-make-and-clock-configuration/

#[entry]
fn main() -> ! {
    // Setup handler for device peripherals
    let dp = pac::Peripherals::take().unwrap();

    // Enable HSE Clock
    dp.RCC.cr.write(|w| w.hseon().set_bit());

    // Wait for HSE clock to become ready
    while dp.RCC.cr.read().hserdy().is_ready() {}

    // Set the power enable clock and voltage regulator
    dp.RCC.apb1enr.write(|w| w.pwren().enabled());
    delay(10);
    // Regulator voltage scaling output selection,
    // needed for high clock speed
    // need to turn pll off first
    dp.RCC.cr.write(|w| w.pllon().off());
    // wait for pll to be off

    info!("waiting for pll to turn off");
    while !dp.RCC.cr.read().pllrdy().is_not_ready() {}

    const SCALE_TWO: u8 = 0b10;
    unsafe {
        dp.PWR.cr.write(|w| w.vos().bits(SCALE_TWO));
    }

    // wait for VOS to become ready
    info!("waiting for voltage output selection to be ready");
    while !dp.PWR.csr.read().vosrdy().bit_is_set() {}

    // enable instruction cache, prefetch buffer
    // data cache and set flash latency to 5 wait states
    dp.FLASH.acr.write(|w| w.icen().set_bit());
    dp.FLASH.acr.write(|w| w.prften().set_bit());
    dp.FLASH.acr.write(|w| w.dcen().set_bit());
    dp.FLASH.acr.write(|w| w.latency().ws5());

    // Configure bus prescalars
    dp.RCC.cfgr.write(|w| w.hpre().div1()); // AHB prescalar
    dp.RCC.cfgr.write(|w| unsafe { w.ppre1().bits(2) });
    dp.RCC.cfgr.write(|w| unsafe { w.ppre2().bits(1) });

    unsafe {
        dp.RCC.pllcfgr.write(|w| w.pllsrc().hse());

        const PLLM: u8 = 25;
        dp.RCC.pllcfgr.write(|w| w.pllm().bits(PLLM));

        const PLLN: u16 = 168;
        dp.RCC.pllcfgr.write(|w| w.plln().bits(PLLN));

        const PLLP: u8 = 2;
        dp.RCC.pllcfgr.write(|w| w.pllp().bits(PLLP));
    }

    dp.RCC.cr.write(|w| w.pllon().off());

    info!("waiting for PLL to become ready");
    while !dp.RCC.cr.read().pllrdy().bit_is_set() {}

    // Select PLL as System Clock Source
    dp.RCC.cfgr.write(|w| w.sw().pll());

    info!("waiting for PLL to be selected as System Clock Source");
    while !dp.RCC.cfgr.read().sws().is_pll() {}

    // pll div5 measures 3.2Mhz (* 5 = 16 Mhz)
    // hse div5 also measures 3.2Mhz (* 5 = 16 Mhz)

    // enable MCO1 HSE output
    dp.RCC.cfgr.write(|w| w.mco1().pll());
    dp.RCC.cfgr.write(|w| w.mco1pre().div5());

    //Enable Clock to GPIOA
    dp.RCC.ahb1enr.write(|w| w.gpioaen().set_bit());

    //Configure PA5 as Output
    dp.GPIOA.moder.write(|w| w.moder8().output());
    // dp.GPIOA.otyper.write(|w| w.ot8().push_pull());

    // // Set PA5 Output to High signalling end of configuration
    // dp.GPIOA.odr.write(|w| w.odr8().low());

    dp.GPIOA.ospeedr.write(|w| w.ospeedr8().very_high_speed());
    dp.GPIOA.moder.write(|w| w.moder8().alternate());
    dp.GPIOA.afrh.write(|w| w.afrh8().af0());
    info!("ready");

    loop {}
}
