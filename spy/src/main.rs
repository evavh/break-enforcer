#![no_main]
#![no_std]

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
// global logger
use panic_probe as _;

use stm32f4xx_hal as hal; // includes memory.x?

use crate::hal::{pac::interrupt, prelude::_stm32f4xx_hal_gpio_ExtiPin};

use core::{
    arch::asm,
    sync::atomic::{AtomicBool, Ordering},
};
use cortex_m_rt::entry;

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

const ARRAY_LEN: usize = 240;
static mut ARRAY: [u32; ARRAY_LEN] = [0u32; ARRAY_LEN];
static DONE: AtomicBool = AtomicBool::new(false);

// stm32f4 has minimum 12 cycle interrupt delay
// usb clocks at 12Mhz, stm at 84Mhz
// stm has 7 cycles for each usb cycle
// that means this IRQ misses at least the first 2
// usb clock cycles
//
// The IRQ uses polling to get the data faster. After
// the IRQ the data will be analyzed

// put interrupt code in ram (.data is kept in ram)
#[link_section = ".data.EXTI1"]
#[interrupt]
fn EXTI1() {
    unsafe {
        let idr = &(*hal::pac::GPIOB::ptr()).idr;
        // need to manually unroll loop to get correct timings
        *ARRAY.get_unchecked_mut(1) = idr.read().bits();
        asm!( "
              NOP
              NOP
              NOP
        " );
        *ARRAY.get_unchecked_mut(2) = idr.read().bits();
        asm!( "
              NOP
              NOP
              NOP
        " );
        // *ARRAY.get_unchecked_mut(3) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(4) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(5) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(6) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(7) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(8) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(9) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(10) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(11) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(12) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(13) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(14) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(15) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(16) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(17) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(18) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(19) = idr.read().bits();
        // *ARRAY.get_unchecked_mut(20) = idr.read().bits();
        // for i in 0..ARRAY_LEN {
        //     *ARRAY.get_unchecked_mut(i) = idr.read().bits();
        // }
        // set interrupt as handled
        (*hal::pac::EXTI::ptr()).pr.write(|w| w.pr1().set_bit());
    }
    DONE.store(true, Ordering::Relaxed);
}

#[entry]
fn main() -> ! {
    let mut dp = Peripherals::take().unwrap();
    let rcc = dp.RCC.constrain();
    let _clocks = rcc
        .cfgr
        .sysclk(84.MHz())
        .use_hse(84.MHz())
        .hclk(84.MHz()) // also called AHB clock
        // APB2 clock; data on I/O pin is sampled into
        // this every APB2 clock cycle
        .pclk2(84.MHz())
        .freeze();

    // set power on usb (prototype needs this)
    let gpio_c = dp.GPIOC.split();
    let mut usb_enable = gpio_c.pc13.into_push_pull_output();
    usb_enable.set_high();

    let gpio_b = dp.GPIOB.split();
    let mut usb = gpio_b.pb1.into_floating_input();
    let usb_pin = usb.pin_id();

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

    let mut arrayarray: [[u32; 240]; 10] = [[0u32; 240]; 10];
    loop {
        for a in &mut arrayarray {
            while !DONE
                .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {}

            unsafe {
                *a = ARRAY.clone();
            }
        }

        info!("got arrayyyysss");

        for a in arrayarray {
            let mut ones_and_zeros = ['0'; 240];
            let package = Package::new(a, usb_pin as usize);
            info!("{}", package);
        }
    }
    exit()
}

enum Package {
    Long,
    Short,
    Unknown { bytes: [u8; 240] },
}

impl Package {
    fn new(bytes: [u32; 240], usb_pin: usize) -> Package {
        let bytes = bytes.map(|port| port & (1 << usb_pin)).map(|b| b as u8);

        if bytes == package::LONG {
            return Self::Long;
        }
        if bytes == package::SHORT {
            return Self::Short;
        }
        Package::Unknown { bytes }
    }
}

impl defmt::Format for Package {
    fn format(&self, fmt: defmt::Formatter) {
        match self {
            Package::Long => defmt::write!(fmt, "Long"),
            Package::Short => defmt::write!(fmt, "Short"),
            Package::Unknown { bytes } => {
                defmt::write!(fmt, "Unknown, bytes: [");
                for b in bytes {
                    if *b == 0 {
                        defmt::write!(fmt, "0");
                    } else {
                        defmt::write!(fmt, "1");
                    }
                }
                defmt::write!(fmt, "]\n");
            }
        }
    }
}

#[rustfmt::skip]
mod package {
    pub const LONG: [u8; 240] = [
        1,0,1,1,0,0,0,0,0,0,1,1,0,0,0,0,1,1,1,1,1,1,0,0,0,0,1,1,1,1,1,1,1,1,1,0,0,0,1,1,0,1,1,1,1,1,1,1,1,0,0,0,0,0,1,1,1,1,1,1,0,1,1,0,0,1,1,0,0,1,1,0,0,0,0,1,1,1,1,0,0,0,0,0,0,1,1,1,1,0,0,0,0,0,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,0,0,1,1,0,1,1,0,1,1,0,0,0,1,1,0,0,0,0,1,1,1,1,1,1,0,0,0,0,1,1,1,1,1,1,1,1,1,1,0,0,1,1,1,1,0,1,1,0,1,1,1,1,1,1,0,0,0,1,1,1,1,1,1,1,1,0,1,1,0,1,0,1,1,0,0,0,0,1,1,1,1,0,0,0,0,0,0,1,1,1,1,0,0,0,0,0,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    ];

    pub const SHORT: [u8; 240] = [
        1,0,1,1,0,0,0,0,0,0,1,1,1,1,0,0,1,1,1,1,0,0,0,0,0,0,1,1,0,0,1,1,1,1,1,0,0,1,1,1,1,1,1,1,1,1,0,0,0,1,1,0,0,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,0,0,1,1,1,0,1,1,1,0,1,1,1,0,0,0,0,0,1,1,0,0,1,1,1,1,1,0,0,1,0,0,0,0,1,0,0,1,0,0,0,0,0,0,1,1,1,1,1,1,0,0,0,0,0,0,0,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    ];
}
