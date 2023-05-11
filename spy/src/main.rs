#![no_main]
#![no_std]

use defmt::info;
use defmt_rtt as _;
use hal::{
    gpio::{self, Pin},
    pac::Peripherals,
    prelude::_stm32f4xx_hal_gpio_GpioExt,
    syscfg::SysCfgExt,
};
// global logger
use panic_probe as _;

use stm32f4xx_hal as hal; // includes memory.x?

use crate::hal::{pac::interrupt, prelude::_stm32f4xx_hal_gpio_ExtiPin};

use core::{
    cell::Cell,
    sync::atomic::{AtomicBool, Ordering},
};
use cortex_m::interrupt::Mutex;
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

type UsbData = Pin<'B', 1>;
static ARRAY: Mutex<[bool; 80]> = Mutex::new([false; 80]);
static USB: Mutex<Cell<Option<UsbData>>> = Mutex::new(Cell::new(None));
static DONE: AtomicBool = AtomicBool::new(false);

unsafe fn force_mut<T>(reference: &T) -> &mut T {
    let const_ptr = reference as *const T;
    let mut_ptr = const_ptr as *mut T;
    &mut *mut_ptr
}

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
    cortex_m::interrupt::free(|cs| {
        let array = unsafe { force_mut(ARRAY.borrow(cs)) };
        let usb = unsafe {
            force_mut(USB.borrow(cs))
                .get_mut()
                .as_mut()
                .unwrap_unchecked()
            // .expect("pin is set before ISR is enabled")
        };
        usb.clear_interrupt_pending_bit();
        for m in array {
            *m = usb.is_high();
        }
    });
    DONE.store(true, Ordering::Relaxed);
}

#[entry]
fn main() -> ! {
    let mut dp = Peripherals::take().unwrap();

    // set power on usb (prototype needs this)
    let gpio_c = dp.GPIOC.split();
    let mut usb_enable = gpio_c.pc13.into_push_pull_output();
    usb_enable.set_high();

    let gpio_b = dp.GPIOB.split();
    let mut usb = gpio_b.pb1.into_floating_input();

    let mut syscfg = dp.SYSCFG.constrain();
    usb.make_interrupt_source(&mut syscfg);
    usb.enable_interrupt(&mut dp.EXTI);
    usb.trigger_on_edge(&mut dp.EXTI, gpio::Edge::Falling);
    let interrupt_number = usb.interrupt();

    cortex_m::interrupt::free(|cs| USB.borrow(cs).swap(&Cell::new(Some(usb))));

    // info!("{:?}", interrupt_number);
    // clear pending interrupts on usb gpio
    cortex_m::peripheral::NVIC::unpend(interrupt_number);
    unsafe {
        // enable interrupt on usb gpio
        cortex_m::peripheral::NVIC::unmask(interrupt_number);
    }

    loop {
        while !DONE
            .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {}
        let mut array = [false; 80];
        cortex_m::interrupt::free(|cs| {
            let shared = ARRAY.borrow(cs);
            array = shared.clone();
        });

        info!("array: {}", array);
    }
    exit()
}
