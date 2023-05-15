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
use stm32f4xx_hal as hal;
// global logger
use panic_probe as _;

use crate::hal::prelude::_stm32f4xx_hal_gpio_ExtiPin;

use core::sync::atomic::{AtomicBool, Ordering};
use cortex_m_rt::entry;

mod read_packets;

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

static mut GPIO_STATE_PTR: u32 = 0;
static mut ARRAY: [u32; 20] = [0u32; 20];
const ARRAY_LEN: usize = 20;
static DONE: AtomicBool = AtomicBool::new(false);

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
    info!("usb bin: {}", usb_pin);
    unsafe { GPIO_STATE_PTR = (*crate::hal::pac::GPIOB::ptr()).idr.as_ptr() as u32 };
    info!("pin addr: {:x}", unsafe { GPIO_STATE_PTR });

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

    let mut arrayarray: [[u32; ARRAY_LEN]; 10] = [[0u32; ARRAY_LEN]; 10];
    loop {
        info!("hi");
        for a in &mut arrayarray {
            while !DONE
                .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {}

            unsafe {
                *a = ARRAY.clone();
            }
        }

        for a in arrayarray {
            let package = Package::new(a, usb_pin as usize);
            info!("{}", package);
        }
    }
    // exit()
}

enum Package {
    Long,
    Short,
    Unknown { bytes: [u8; ARRAY_LEN] },
}

impl Package {
    fn new(bytes: [u32; ARRAY_LEN], usb_pin: usize) -> Package {
        let bytes = bytes.map(|port| port & (1 << usb_pin)).map(|b| b as u8);

        // if bytes == package::LONG {
        //     return Self::Long;
        // }
        // if bytes == package::SHORT {
        //     return Self::Short;
        // }
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
