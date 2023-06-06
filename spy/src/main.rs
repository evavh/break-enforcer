#![no_main]
#![no_std]
#![feature(ptr_sub_ptr)]
#![feature(array_zip)]
#![feature(slice_partition_dedup)]
#![feature(asm_const)]
#![feature(const_option)]
#![feature(generic_const_exprs)]

use defmt::{info, warn};
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

use crate::{decoder::mask, hal::prelude::_stm32f4xx_hal_gpio_ExtiPin};

use buffering::SwapBufReader;
use core::{arch::asm, array, ptr, sync::atomic::AtomicUsize};
use cortex_m_rt::entry;

// mod debug_pulse;
mod read_packets;

mod packet;
use packet::Packet;

mod decoder;

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

const ARRAY_LEN: usize = 200;
static mut ARRAY_STORE: [[u32; ARRAY_LEN]; 4] = [[0; ARRAY_LEN]; 4];
static NEXT: AtomicUsize = AtomicUsize::new(0);

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
    let mut usb_data_plus = gpio_b.pb1.into_floating_input();
    let mut _usb_data_min = gpio_b.pb2.into_floating_input();

    info!("usb pin: pb{}", usb_data_plus.pin_id());
    // get adress of GPIOB's IDR (input data) register. Accessed as 32 bit
    // word, however only the lower 16 bit represent pin values
    let usb_data_register = unsafe { (*hal::pac::GPIOB::ptr()).idr.as_ptr() };
    info!(
        "data plus: pb{}, data min: pb{}, addr (PB in): {:x}",
        usb_data_plus.pin_id(),
        _usb_data_min.pin_id(),
        usb_data_register
    );
    let debug_out = unsafe { (*hal::pac::GPIOC::ptr()).odr.as_ptr() };
    info!("debug out (pa0) addr (PC out): {:x}", debug_out);

    // exit();

    let mut syscfg = dp.SYSCFG.constrain();
    usb_data_plus.make_interrupt_source(&mut syscfg);
    usb_data_plus.enable_interrupt(&mut dp.EXTI);
    usb_data_plus.trigger_on_edge(&mut dp.EXTI, gpio::Edge::Falling);
    let interrupt_number = usb_data_plus.interrupt();

    let reader = SwapBufReader {
        next: &NEXT,
        raw: unsafe { &ARRAY_STORE },
    };
    let mut packets: Packets<50, ARRAY_LEN> = Packets::new();

    // clear pending interrupts on usb gpio
    cortex_m::peripheral::NVIC::unpend(interrupt_number);
    unsafe {
        // enable interrupt on usb gpio
        cortex_m::peripheral::NVIC::unmask(interrupt_number);
        core.NVIC.set_priority(usb_data_plus.interrupt(), 0); // set highest prio
    }

    // wait for 1 seconds (assuming 84Mhz)
    // cortex_m::asm::delay(84 * 1_000_000 * 1);
    // packets.collect(reader);
    // info!("{}", packets.list);
    // info!("{}", packets.hashes);

    loop {}
}

struct PacketDecoder<'a, const N: usize, const LEN: usize>
where
    [(); (LEN + 31) / 32]:,
{
    packets: &'a mut Packets<N, LEN>,
    free_before_attempt: usize,
    lost: usize,
}

impl<'a, const N: usize, const LEN: usize> PacketDecoder<'a, N, LEN>
where
    [(); (LEN + 31) / 32]:,
{
    fn full(&self) -> bool {
        self.packets.free == self.packets.list.len()
    }
}

impl<'a, const N: usize, const LEN: usize> buffering::DataHandler<LEN> for PacketDecoder<'a, N, LEN>
where
    [(); (LEN + 31) / 32]:,
{
    fn attempt(&mut self, data: &[u32; LEN]) {
        self.free_before_attempt = self.packets.free;
        self.packets.append(data);
    }
    fn mark_success(&mut self) {}
    fn mark_corrupt(&mut self) {
        self.packets.free = self.free_before_attempt;
        self.packets.list[self.packets.free].reset();
    }
    fn lost(&mut self, n: usize) {
        self.lost += n;
    }
}

struct Packets<const N: usize, const LEN: usize>
where
    [(); (LEN + 31) / 32]:,
{
    list: [Packet<LEN>; N],
    hashes: [u32; N],
    free: usize,
}

impl<const N: usize, const LEN: usize> Packets<N, LEN>
where
    [(); (LEN + 31) / 32]:,
{
    fn new() -> Self {
        Packets {
            list: array::from_fn(|_| Packet::new()),
            hashes: [0u32; N],
            free: 0,
        }
    }

    /// # Panics
    /// function panics if packets is full. You need to
    /// check that before calling this
    fn append(&mut self, candidate: &[u32]) {
        let packet = &mut self.list[self.free];
        let packet_len = candidate[0] as usize;
        for register in &candidate[1..packet_len] {
            let sample = mask::<1>(*register);
            packet.push(sample)
        }
        self.free += 1;
    }

    fn collect<const DEPTH: usize>(&mut self, reader: SwapBufReader<LEN, DEPTH>) {
        let mut num: usize = 0;
        let mut decoder = PacketDecoder {
            free_before_attempt: 0,
            packets: self,
            lost: 0,
        };
        while !decoder.full() {
            // should hang until there is data
            reader.read((), &mut num, &mut decoder);
        }
        if decoder.lost > 0 {
            warn!("Lost {} packages", decoder.lost);
        }
    }
}
