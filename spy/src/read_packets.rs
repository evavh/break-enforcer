use crate::hal::pac::interrupt;

use core::{arch::asm, sync::atomic::Ordering};
use stm32f4xx_hal as hal;

use crate::{ARRAY, DONE};

// stm32f4 has minimum 12 cycle interrupt delay
// usb clocks at 12Mhz, stm at 84Mhz
// stm has 7 cycles for each usb cycle
// that means this IRQ misses at least the first 2
// usb clock cycles
//
// The IRQ uses polling to get the data faster. After
// the IRQ the data will be analyzed

// put interrupt code in ram (.data is kept in ram)
//
// https://developer.arm.com/documentation/100166/0001/Programmers-Model/Instruction-set-summary/Table-of-processor-instructions
//
#[link_section = ".data.EXTI1"]
#[interrupt]
fn EXTI1() {
    unsafe {
        // let idr = &(*hal::pac::GPIOB::ptr()).idr;
        // *ARRAY.get_unchecked_mut(1) = idr.read().bits();
        asm!(
            // Prepare registers
            //
            // pin adress in r0
            "movw r0, #1040",                             // 1 cycle
            // keep start of ARRAY in r2
            "movw r2, :lower16:break_spy::ARRAY",         // 1 cycle
            // continue moving pin addr to r0
            "movt r0, #16386",                            // 1 cycle
            // continue putting ARRAY in r2
            "movt r2, :upper16:break_spy::ARRAY",        // 1 cycle


            // Store gpio value in ARRAY
            //
            // load the current value of the pin into r1
            "ldr r1, [r0]",                              // 2 cycles
            // store r1 in the adress pointed to by r2 with 4 added
            "str r1, [r2, #4]",                          // 2 cycles
            "NOP",                                       // 1 cycle
            "NOP",                                       // 1 cycle
            "NOP",                                       // 1 cycle
            // = 7 cycles

            // Set interrupt as handled
            //
            // set memory adress of interrupt pending in r0
            "movw r0, #15380",                           // 1 cycle
            "movt r0, #16385",                           // 1 cycle
            // set interrupt pending to 2/confirm handled 
            "movs r1, #2",                               // 1 cycle
            "str r1, [r0]",                              // 2 cycles

            // Mark as done for non interrupt code
            //
            // set r0 to break_spy the adress of DONE
            "movw r0, :lower16:break_spy::DONE",         // 1 cycle
            "movt r0, :upper16:break_spy::DONE",         // 1 cycle
            // set r1 to the pr1 bit (1)
            "mov r1, #1",                                // 1 cycle
            // set DONE to r1 (which is 1)
            "strb r1, [r0]",                             // 2 cycles
        );
        // set interrupt pending to false/confirm interrupt handled
        // (*hal::pac::EXTI::ptr()).pr.write(|w| w.pr1().set_bit());
    }
    // signal packet rdy to code after interrupt
    // DONE.store(true, Ordering::Relaxed);
}
