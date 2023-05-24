#![no_main]
#![no_std]

use core::arch::asm;

use cortex_m::asm::delay;
use defmt::info;
use defmt_rtt as _;
use hal::pac;
use stm32f4xx_hal as hal;
// global logger
use panic_probe as _;
use cortex_m_rt::entry;

// based on: https://controllerstech.com/stm32-clock-setup-using-registers/
// https://kleinembedded.com/stm32-without-cubeide-part-2-cmsis-make-and-clock-configuration/

// c hal sources:
// https://github.com/zephyrproject-rtos/hal_stm32/blob/c865374fc83d93416c0f380e6310368ff55d6ce2/stm32cube/stm32f0xx/drivers/include/stm32f0xx_hal_rcc.h#L751

#[entry]
fn main() -> ! {
    // Setup handler for device peripherals
    let dp = pac::Peripherals::take().unwrap();

    // Enable HSE Clock
    dp.RCC.cr.write(|w| w.hseon().set_bit());

    // Wait for HSE clock to become ready
    while dp.RCC.cr.read().hserdy().is_not_ready() {}

    // Set the power interface clock to on
    dp.RCC.apb1enr.write(|w| w.pwren().enabled());
    delay(10);

    // enable instruction cache, prefetch buffer
    // data cache and set flash latency to 5 wait states
    dp.FLASH.acr.write(|w| {
        w.icen().set_bit();
        w.prften().set_bit();
        w.dcen().set_bit();
        w.latency().ws5()
    });

    // Configure bus prescalars
    unsafe {
        dp.RCC.cfgr.write(|w| {
            w.hpre().div1(); // AHB prescalar
            w.ppre1().bits(2);
            w.ppre2().bits(1)
        })
    }

    const PLLM: u8 = 25;
    const PLLN: u16 = 168;
    const PLLP_DIV_2: u8 = 0;
    unsafe {
        dp.RCC.pllcfgr.write(|w| {
            w.pllsrc().hse();
            w.pllm().bits(PLLM);
            w.plln().bits(PLLN);
            w.pllp().bits(PLLP_DIV_2)
        });
    }

    // turn on pll
    dp.RCC.cr.write(|w| {
        w.hseon().on();
        w.pllon().on()
    });

    const SCALE_TWO: u8 = 0b10;
    unsafe {
        dp.PWR.cr.write(|w| w.vos().bits(SCALE_TWO));
    }
    info!("waiting for voltage output selection to be ready");
    while dp.PWR.csr.read().vosrdy().bit_is_clear() {}

    info!("waiting for PLL to become ready");
    while dp.RCC.cr.read().pllrdy().is_not_ready() {}

    // Select PLL as System Clock Source
    unsafe {
        dp.RCC.cfgr.write(|w| {
            w.sw().pll();
            w.hpre().div1(); // AHB prescalar
            w.ppre1().bits(2);
            w.ppre2().bits(1)
        });
    }

    info!("waiting for PLL to be selected as System Clock Source");
    while !dp.RCC.cfgr.read().sws().is_pll() {}

    //Enable Clock to GPIOA for mco1 and debug pin
    dp.RCC.ahb1enr.write(|w| w.gpioaen().set_bit());

    dp.GPIOA.moder.write(|w| {
        w.moder0().output();
        w.moder8().alternate()
    });
    dp.GPIOA.afrh.write(|w| w.afrh8().af0());

    // enable MCO1 HSE output
    // pll div5 measures 3.2Mhz (* 5 = 16 Mhz)
    // hse div5 also measures 3.2Mhz (* 5 = 16 Mhz)
    unsafe {
        dp.RCC.cfgr.write(|w| {
            w.mco1pre().div5();
            w.mco1().pll();
            w.sw().pll();
            w.hpre().div1(); // AHB prescalar
            w.ppre1().bits(2);
            w.ppre2().bits(1)
        });
    }

    info!("ready");

    //Configure PA as Output
    dp.GPIOA.otyper.write(|w| w.ot0().push_pull());
    dp.GPIOA.ospeedr.write(|w| w.ospeedr0().very_high_speed());

    loop {
        dp.GPIOA.odr.write(|w| w.odr0().low());
        unsafe {
            asm!{
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
                "NOP",
            }
        }
        dp.GPIOA.odr.write(|w| w.odr0().high());
    }
}
