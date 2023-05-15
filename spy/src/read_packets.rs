use crate::{ARRAY, DONE};
use core::arch::global_asm;

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

// Note: only use R0,R1,R2,R3 and R12. Others are not saved by the
// mcu before entering the interrupt.
global_asm! {
    ".section .data.EXTI1",
    ".global EXTI1",
    ".p2align 1",
    ".type EXTI1,%function",
    ".code 16",
    ".thumb_func",
"EXTI1:",
    ".fnstart",
    ".cfi_startproc",

    // Set the pin state adress in r0
    "movw r0, #1040",                            // 1 cycle
    "movt r0, #16386",                           // 1 cycle
                                                 
    // reads the pin state ASAP
    "ldr r1, [r0]",                              // 2 cycles
    // build the pointer to ARRAY in r2 so we can store it
    "movw r2, :lower16:{ARRAY}",                 // 1 cycle
    "movt r2, :upper16:{ARRAY}",                 // 1 cycle

    // store the pinstate in ARRAY[0]
    "str r1, [r2, #0]",                          // 2 cycles
    "NOP",                                       // 1 cycle
    // = 7 cycles after first read



    // store pin state in ARRAY[1] 
    // and prepare to set interrupt pending to false
    "ldr r1, [r0]",                              // 2 cycles
    "str r1, [r2, #4]",                          // 2 cycles
    // build the pointer to interrupt handled in r3
    "movw r3, #15380",                           // 1 cycle
    "movt r3, #16385",                           // 1 cycle
    "NOP",
    // = 14 cycles after first read



    // store pin state in ARRAY[2]
    // and finish setting interrupt pending to false
    // set interrupt pending to 2/confirm handled
    "ldr r1, [r0]",                              // 2 cycles
    "str r1, [r2, #8]",                          // 2 cycles
    // set 2nd (r4) irq_handled bit (at r3) to true
    "movs r4, #2",                               // 1 cycle
    "str r4, [r3]",                              // 2 cycles
    // = 21 cycles after first read



    // store pin state in ARRAY[3]
    // and prepare to set data rdy boolean to true
    "ldr r1, [r0]",                              // 2 cycles
    "str r1, [r2, #12]",                         // 2 cycles
    "movw r3, :lower16:{DONE}",                  // 1 cycle
    "movt r3, :upper16:{DONE}",                  // 1 cycle
    "NOP",                                       // 1 cycle
    // = 28 cycles after first read
                                                 
    // store pin state in ARRAY[4]
    // and finish setting data rdy boolean to true
    "ldr r1, [r0]",                              // 2 cycles
    "str r1, [r2, #16]",                         // 2 cycles
    // set DONE (r3) to 1 (r4)
    "mov r4, #1",                                // 1 cycle
    "strb r4, [r3]",                             // 2 cycles
    // = 35 cycles after first read


    // Store gpio value in ARRAY (repeat N-5 times) 
    // Because:
    //  - the first read is combined with setting up the array pointer
    //  - the second+third read is combined with setting the interrupt as handled
    //  - the fourth+fifth read sets the new data bool to true
    // include_str!(concat!(env!("OUT_DIR"), "/loop.s")),

    // return out of the interrupt
    "bx lr",                                     // 1 cycle minimum

    ".cfi_endproc",
    ".cantunwind",
    ".fnend",

    ARRAY = sym ARRAY,
    DONE = sym DONE,
}
