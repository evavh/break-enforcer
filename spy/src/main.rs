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
    pac::{CorePeripherals, Peripherals},
    prelude::_stm32f4xx_hal_gpio_GpioExt,
    rcc::RccExt,
    syscfg::SysCfgExt,
};
use stm32f4xx_hal as hal;
// global logger
use panic_probe as _;

use crate::hal::prelude::_stm32f4xx_hal_gpio_ExtiPin;

use core::{
    arch::asm,
    ptr, slice,
    sync::atomic::{AtomicBool, Ordering},
};
use cortex_m_rt::entry;

mod debug_pulse;
// mod read_packets;
// use read_packets::ARRAY_OFFSET;

/// Terminates the application and makes `probe-run` exit with exit-code = 0
pub fn exit() -> ! {
    loop {
        cortex_m::asm::bkpt();
    }
}

#[no_mangle]
pub unsafe extern "C" fn CustomReset() -> ! {
    extern "C" {
        static mut _evect_in_ram: u8;
        static mut _svect_in_ram: u8;
        static mut _svect_in_flash: u8;
    }
    let count = &_evect_in_ram as *const u8 as usize - &_svect_in_ram as *const u8 as usize;
    ptr::copy_nonoverlapping(
        &_svect_in_flash as *const u8,
        &mut _svect_in_ram as *mut u8,
        count,
    );

    // set vector table offset
    asm!(
        "
        ldr r0, =0xe000ed08        // adress of the VTOR register
        ldr r1, =__vector_table    // new vt location
        str r1, [r0]               // move the vt adress into vtor register
    "
    );

    // Call the cortex-rt reset
    extern "C" {
        fn Reset() -> !;
    }

    Reset()
}

// is transformed into immediate in assembly
const ARRAY_LEN: usize = 360;
// *2 as there are two 'arrays' between which we alternate
static mut ARRAY1: [u32; ARRAY_LEN] = [5u32; ARRAY_LEN];
static mut ARRAY2: [u32; ARRAY_LEN] = [5u32; ARRAY_LEN];
static mut ARRAY_OFFSET: *const u32 = unsafe { ARRAY1.as_ptr() };

static DONE: AtomicBool = AtomicBool::new(false);

#[entry]
fn main() -> ! {
    assert_no_duplicate_patterns();

    let mut dp = Peripherals::take().unwrap();
    let mut core = CorePeripherals::take().unwrap();
    let rcc = dp.RCC.constrain();
    let _clocks = rcc.cfgr.use_hse(25.MHz()).sysclk(84.MHz()).freeze();

    // set power on usb (prototype needs this)
    let gpio_c = dp.GPIOC.split();
    let mut usb_enable = gpio_c.pc13.into_push_pull_output();
    usb_enable.set_high();

    // set debug pin (pa0) to fast output
    let gpio_a = dp.GPIOA.split();
    let mut debug_pin = gpio_a.pa0.into_push_pull_output();
    debug_pin.set_speed(gpio::Speed::VeryHigh);
    debug_pin.set_low();

    unsafe {
        // output the high speed external clock on PA8
        hal::pac::Peripherals::steal().RCC.cfgr.modify(|_, w| {
            w.mco1().pll();
            w.mco1pre().div5()
        });
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
        core.NVIC.set_priority(usb.interrupt(), 0); // set highest prio
    }

    // let mut counter = 0;
    let mut arrayarray: [[u32; ARRAY_LEN]; 15] = [[0u32; ARRAY_LEN]; 15];
    loop {
        for a in &mut arrayarray {
            while !DONE
                .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                // counter += 1;
                // if counter % 4000 == 0 {
                //     trace!("waiting for interrupt");
                // }
            }

            unsafe {
                let array = slice::from_raw_parts(ARRAY_OFFSET, ARRAY_LEN);
                a.clone_from_slice(array)
            }
        }

        for a in arrayarray {
            let package = Package::new(a, usb_pin as usize);
            if let Package::Unknown { meta, .. } = package {
                info!("{}", meta);
            }
        }
    }
    // exit()
}

#[derive(defmt::Format, Clone)]
struct Meta {
    ones: usize,
    len: Option<usize>,
}

impl Meta {
    fn from_msg(msg: &[u8]) -> Self {
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

        Self { len, ones }
    }

    fn distance(&self, other: &Self) -> usize {
        let (Some(self_len), Some(other_len)) = (self.len, other.len) else {
            return 1000;
        };
        self.ones.abs_diff(other.ones) + self_len.abs_diff(other_len)
    }

    fn similar_to(&self, other: &Self) -> bool {
        self.distance(other) < 5
    }
}

enum Package {
    Known(usize),
    Unknown { meta: Meta, msg: [u8; ARRAY_LEN] },
}

impl Package {
    fn new(port_bytes: [u32; ARRAY_LEN], usb_pin: usize) -> Package {
        let msg = port_bytes
            .map(|port| port >> usb_pin) // shift back so 0 or 2 becomes 0 or 1
            .map(|port| port & 1) // everything non 1 becomes zero
            .map(|b| b as u8);

        let meta = Meta::from_msg(&msg);
        for (idx, pattern) in KNOWN.iter().enumerate() {
            if meta.similar_to(&pattern) {
                return Package::Known(idx);
            }
        }

        Package::Unknown { meta, msg }
    }
}

impl defmt::Format for Package {
    fn format(&self, fmt: defmt::Formatter) {
        match self {
            Package::Known(c) => defmt::write!(fmt, "Known: {}", c),
            Package::Unknown { meta, msg } => {
                defmt::write!(fmt, "Unknown (meta: {}), bytes: [", meta);
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

#[rustfmt::skip]
const KNOWN: [Meta; 52] = [
	Meta { ones: 135, len: Some(240) },
	Meta { ones: 39, len: Some(69) },
	Meta { ones: 44, len: Some(69) },
	Meta { ones: 204, len: Some(235) },
	Meta { ones: 140, len: Some(253) },
	Meta { ones: 44, len: Some(69) },
	Meta { ones: 207, len: Some(234) },
	Meta { ones: 148, len: Some(251) },
	Meta { ones: 43, len: Some(68) },
	Meta { ones: 203, len: Some(235) },
	Meta { ones: 41, len: Some(69) },
	Meta { ones: 155, len: Some(253) },
	Meta { ones: 203, len: Some(235) },
	Meta { ones: 142, len: Some(253) },
	Meta { ones: 204, len: Some(235) },
	Meta { ones: 146, len: Some(253) },
	Meta { ones: 45, len: Some(69) },
	Meta { ones: 206, len: Some(235) },
	Meta { ones: 142, len: Some(253) },
	Meta { ones: 40, len: Some(69) },
	Meta { ones: 207, len: Some(235) },
	Meta { ones: 142, len: Some(253) },
	Meta { ones: 35, len: Some(53) },
	Meta { ones: 37, len: Some(69) },
	Meta { ones: 202, len: Some(234) },
	Meta { ones: 42, len: Some(69) },
	Meta { ones: 140, len: Some(253) },
	Meta { ones: 202, len: Some(235) },
	Meta { ones: 153, len: Some(253) },
	Meta { ones: 205, len: Some(235) },
	Meta { ones: 147, len: Some(253) },
	Meta { ones: 39, len: Some(69) },
	Meta { ones: 44, len: Some(69) },
	Meta { ones: 200, len: Some(235) },
	Meta { ones: 141, len: Some(253) },
	Meta { ones: 43, len: Some(69) },
	Meta { ones: 37, len: Some(69) },
	Meta { ones: 206, len: Some(234) },
	Meta { ones: 141, len: Some(251) },
	Meta { ones: 45, len: Some(69) },
    Meta { ones: 197, len: Some(233) },
    Meta { ones: 156, len: Some(257) },
    Meta { ones: 74, len: Some(90) },
    Meta { ones: 15, len: Some(26) },
    Meta { ones: 360, len: None },
    Meta { ones: 134, len: Some(154) },
    Meta { ones: 100, len: Some(120) },
    Meta { ones: 30, len: Some(47) },
    Meta { ones: 137, len: Some(158) },
    Meta { ones: 136, len: Some(251) },
    Meta { ones: 159, len: Some(252) },
    Meta { ones: 213, len: Some(235) },
];

fn assert_no_duplicate_patterns() {
    let mut i = 0;
    let mut pats = KNOWN.map(|m| {
        i += 1;
        (i, m)
    });

    let (_, dups) = pats.partition_dedup_by(|(_, pa), (_, pb)| pa.similar_to(pb));

    for dup in dups.iter() {
        info!("duplicate pattern: {} (starts at 1), {}", dup.0, dup.1);
    }

    if !dups.is_empty() {
        exit();
    }
}
