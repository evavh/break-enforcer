#![no_main]
#![no_std]
#![feature(ptr_sub_ptr)]
#![feature(array_zip)]
#![feature(slice_partition_dedup)]

use defmt::{info, trace};
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

use core::{
    slice,
    sync::atomic::{AtomicBool, Ordering},
};
use cortex_m_rt::entry;

mod read_packets;
use read_packets::ARRAY_OFFSET;

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

// is transformed into immediate in assembly
static mut GPIO_STATE_PTR: *const u32 = 40020410 as *const u32;
const ARRAY_LEN: usize = 360;
// *2 as there are two 'arrays' between which we alternate
static mut ARRAY1: [u32; ARRAY_LEN] = [5u32; ARRAY_LEN];
static mut ARRAY2: [u32; ARRAY_LEN] = [5u32; ARRAY_LEN];

static DONE: AtomicBool = AtomicBool::new(false);

#[entry]
fn main() -> ! {
    // assert_no_duplicate_patterns();

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
    // get adress of GPIOB's IDR (input data) register. Accessed as 32 bit
    // word, however only the lower 16 bit represent pin values
    unsafe { GPIO_STATE_PTR = (*crate::hal::pac::GPIOB::ptr()).idr.as_ptr() };
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

    let mut counter = 0;
    let mut arrayarray: [[u32; ARRAY_LEN]; 15] = [[0u32; ARRAY_LEN]; 15];
    loop {
        for a in &mut arrayarray {
            while !DONE
                .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                counter += 1;
                if counter % 4000 == 0 {
                    trace!("waiting for interrupt");
                }
            }

            unsafe {
                let array = slice::from_raw_parts(ARRAY_OFFSET, ARRAY_LEN);
                a.clone_from_slice(array)
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
    Known(char),
    Unknown {
        ones: usize,
        len: Option<usize>,
        msg: [u8; ARRAY_LEN],
    },
}

impl Package {
    fn new(port_bytes: [u32; ARRAY_LEN], usb_pin: usize) -> Package {
        let msg = port_bytes
            .map(|port| port >> usb_pin) // shift back so 0 or 2 becomes 0 or 1
            .map(|port| port & 1) // everything non 1 becomes zero
            .map(|b| b as u8);

        // for (letter, data) in KNOWN {
        //     if data == msg {
        //         return Package::Known(letter);
        //     }
        // }

        let mut len = None;
        for (i, b) in msg.iter().enumerate().rev() {
            if *b == 0 {
                len = Some(i + 1);
                break;
            }
        }

        let ones = msg[0..len.unwrap_or(ARRAY_LEN)]
            .iter()
            .map(|b| *b as usize)
            .sum();

        Package::Unknown { len, ones, msg }
    }
}

impl defmt::Format for Package {
    fn format(&self, fmt: defmt::Formatter) {
        match self {
            Package::Known(c) => defmt::write!(fmt, "Known: {}", c),
            Package::Unknown { msg, ones, len } => {
                defmt::write!(fmt, "Unknown (len: {}, ones: {}), bytes: [", len, ones);
                // for b in msg {
                //     match *b {
                //         0 => defmt::write!(fmt, "0"),
                //         1 => defmt::write!(fmt, "1"),
                //         other => defmt::write!(fmt, "{}", other),
                //     }
                // }
                // defmt::write!(fmt, "]\n");
            }
        }
    }
}

// #[rustfmt::skip]
// pub const KNOWN: [(char, [u8; 180]); 7] = [
//     ('A', [
// 1,0,1,1,0,0,1,1,0,0,0,0,0,0,1,1,1,1,1,0,1,0,1,1,1,1,0,0,0,1,1,1,1,1,1,0,1,1,1,1,0,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
//     ]),
//     ('B', [
// 0,0,1,0,0,0,1,1,1,1,1,1,0,1,0,0,0,1,0,1,0,1,1,0,1,0,0,0,1,0,1,0,1,1,1,1,1,1,0,0,1,1,1,0,1,1,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,
//     ]),
//     ('C', [
// 0,0,0,1,1,0,1,1,1,1,1,0,1,0,1,0,0,0,0,1,1,0,1,1,0,0,1,0,1,1,0,1,0,1,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,
//     ]),
//     ('D', [
// 1,0,1,1,0,1,1,1,0,0,0,1,0,0,1,1,1,1,0,0,1,0,1,1,1,1,0,0,0,1,1,1,1,1,1,0,1,1,0,1,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
//     ]),
//     ('E', [
//      1,0,1,1,0,1,1,1,0,0,0,1,0,0,1,1,1,0,0,0,1,0,1,1,1,1,0,0,0,1,1,1,1,1,1,0,1,1,0,1,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
//     ]),
//     ('F', [
// 0,0,1,0,0,0,1,1,1,1,1,0,1,0,1,0,0,0,0,1,1,0,1,1,0,0,1,1,1,1,0,1,0,0,0,1,0,0,1,1,0,0,0,1,0,0,1,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,0,1,0,0,1,0,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,0,1,1,
//     ]),
//     ('G', [
// 1,0,1,1,0,1,1,1,0,0,0,1,0,0,1,1,1,0,0,0,1,0,1,1,1,1,0,0,0,1,1,1,1,1,1,0,1,1,0,1,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
//     ]),
// ];
//
// fn assert_no_duplicate_patterns() {
//     let mut pats = KNOWN;
//     let (_, dups) = pats.partition_dedup_by_key(|(name, pattern)| pattern.clone());
//
//     for dup in dups.iter() {
//         info!("duplicate pattern: {}", dup.0);
//     }
//
//     if !dups.is_empty() {
//         exit();
//     }
// }
