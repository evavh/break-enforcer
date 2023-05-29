#![no_main]
#![no_std]
#![feature(ptr_sub_ptr)]
#![feature(array_zip)]
#![feature(slice_partition_dedup)]
#![feature(asm_const)]

use defmt::{info, trace};
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
    array,
    hash::Hasher,
    hint::unreachable_unchecked,
    ptr, slice,
    sync::atomic::{AtomicBool, Ordering},
};
use cortex_m_rt::entry;

// mod debug_pulse;
mod read_packets;

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
const ARRAY_LEN: usize = 1_000;
// *2 as there are two 'arrays' between which we alternate
static mut ARRAY1: [u32; ARRAY_LEN] = [5u32; ARRAY_LEN];
static mut ARRAY2: [u32; ARRAY_LEN] = [5u32; ARRAY_LEN];
static mut ARRAY_OFFSET: *const u32 = unsafe { ARRAY1.as_ptr() };

static DONE: AtomicBool = AtomicBool::new(false);

#[entry]
fn main() -> ! {
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

    // wait for 5 seconds (assuming 84Mhz)
    cortex_m::asm::delay(84 * 1_000_000 * 5);
    let mut packets: Packets<50> = Packets::new();
    packets.collect(&DONE);
    // info!("{}", packets.list);
    // info!("{}", packets.hashes);
    exit()
}

#[inline]
fn mask<const P: u16>(register: u32) -> Sample {
    // shift the bit representing the pin of intrested to position 0.
    let port = register >> P;
    // make everything else 1 becomes zero
    let port = port & 1;
    match port {
        0 => Sample::Low,
        1 => Sample::High,
        _ => unsafe { unreachable_unchecked() },
    }
}

fn wait_for_new_data(data_rdy: &AtomicBool) {
    while !data_rdy
        .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {}
}

#[derive(Debug, Clone, Copy)]
enum Sample {
    High = 1,
    Low = 0,
}

/// compact representation of a bit list that
/// can easily be hashed
struct Packet {
    buf: [u32; (ARRAY_LEN + 31) / 32], // if only we had div ceil...
    bits: u16,
}

struct PacketIterator<'a> {
    packet: &'a Packet,
    next_bit: u16,
}

impl<'a> IntoIterator for &'a Packet {
    type Item = Sample;
    type IntoIter = PacketIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        PacketIterator {
            packet: self,
            next_bit: 0,
        }
    }
}

impl<'a> Iterator for PacketIterator<'a> {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_bit >= self.packet.bits {
            return None;
        }

        let sample = self.packet.get(self.next_bit);
        self.next_bit += 1;
        Some(sample)
    }
}

impl Packet {
    const fn new() -> Self {
        Self {
            buf: [0u32; (ARRAY_LEN + 31) / 32],
            bits: 0,
        }
    }

    fn get(&self, idx: u16) -> Sample {
        let byte_idx = idx / 32;
        let bit_idx = idx % 32;
        let word = self.buf[byte_idx as usize];
        let bit = (word >> bit_idx) & 1;
        Sample::from_bit(bit)
    }

    fn push(&mut self, val: Sample) {
        let byte_idx = self.bits / 32;
        let bit_idx = self.bits % 32;
        let mask = (val as u32) << bit_idx;
        self.buf[byte_idx as usize] |= mask;
        self.bits += 1;
    }

    fn hash(&self) -> usize {
        let mut hash = rustc_hash::FxHasher::default();
        for word in self.buf {
            hash.write_u32(word);
        }
        // on 32 bit platforms this is a 32 bit hasher
        hash.finish() as usize
    }
}

struct Packets<const N: usize> {
    list: [Packet; N],
    hashes: [u32; N],
    free: usize,
}

impl<const N: usize> Packets<N> {
    fn new() -> Self {
        Packets {
            list: array::from_fn(|_| Packet::new()),
            hashes: [0u32; N],
            free: 0,
        }
    }

    fn append(&mut self, candidate: &[u32]) -> Result<(), &'static str> {
        let packet = &mut self.list[self.free];
        let mut sum = 0;
        for register in candidate {
            let sample = mask::<1>(*register);
            sum += sample as u16;
            packet.push(sample)
        }
        let hash = packet.hash() as u32;
        let known = self.hashes.contains(&hash);
        assert_ne!(sum, 0);
        if known {
            trace!("known package, hash: {}", hash);
            return Ok(());
        }

        self.hashes[self.free] = hash;
        self.free += 1;
        if self.free >= N {
            Err("no more space for new packets")
        } else {
            Ok(())
        }
    }

    fn collect(&mut self, data_rdy: &AtomicBool) {
        loop {
            wait_for_new_data(data_rdy);
            let array = unsafe { slice::from_raw_parts(ARRAY_OFFSET, ARRAY_LEN) };
            if let Err(_) = self.append(array) {
                info!("Packets storage is full");
                break;
            }
        }
    }
}

impl defmt::Format for Packet {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "\n[");
        for sample in self {
            defmt::write!(fmt, "{}", sample.char());
        }
        defmt::write!(fmt, "]\n");
    }
}

impl Sample {
    fn char(&self) -> char {
        match self {
            Sample::High => '1',
            Sample::Low => '0',
        }
    }

    fn from_bit(bit: u32) -> Self {
        if bit == 0 {
            Sample::Low
        } else {
            Sample::High
        }
    }
}
