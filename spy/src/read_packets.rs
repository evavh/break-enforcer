use crate::{ARRAY, ARRAY_LEN, DONE, GPIO_STATE_PTR};
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


    // // Set the pin state adress in r0
    // "movw r0, #:lower16:{GPIO_STATE_PTR}",       // 1 cycle
    // "movt r0, #:upper16:{GPIO_STATE_PTR}",       // 1 cycle
    "movw r0, #1040",
    "movt r0, #16386",

    // reads the pin state ASAP
    "ldr r1, [r0]",                              // 2 cycles

    // // disable all interrupts
    // "CPSID",
    "NOP",

    // build the pointer to ARRAY in r2 so we can store it
    // it should be the value at memory adress ARRAY_OFFSET
    "movw r3, :lower16:{ARRAY_OFFSET}",          // 1 cycle
    "movt r3, :upper16:{ARRAY_OFFSET}",          // 1 cycle
    "ldr r2, [r3]",                              // 2 cycles
    // store the pinstate in ARRAY[0]
    // = 7 cycles after first read



    // store pin state in ARRAY[1]
    // and prepare to set interrupt pending to false
    "ldr r3, [r0]",                              // 2 cycles
    "str r1, [r2]",                              // 2 cycles
    "str r3, [r2, #4]",                          // 2 cycles
    "NOP",
    // = 14 cycles after first read



    // store pin state in ARRAY[2]
    // and finish setting interrupt pending to false
    // set interrupt pending to 2/confirm handled
    "ldr r1, [r0]",                              // 2 cycles
    "str r1, [r2, #8]",                          // 2 cycles
    // set 2nd (r4) irq_handled bit (at r3) to true
    // TODO: still needed? we do this at the end now 
    // <16-05-23, dvdsk noreply@davidsk.dev>
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
    include_str!(concat!(env!("OUT_DIR"), "/loop.s")),

    // // re-enable interrupts
    // "CPSIE",

    // mark interrupt as no longer pending
    "movw r3, #15380",                           // 1 cycle
    "movt r3, #16385",                           // 1 cycle
    "movs r4, #2",                               // 1 cycle
    "str r4, [r3]",                              // 2 cycles

    "movw r1, :lower16:{ARRAY1}",            // 1 cycle
    "movt r1, :upper16:{ARRAY1}",            // 1 cycle
    "movw r3, :lower16:{ARRAY_OFFSET}",          // 1 cycle
    "movt r3, :upper16:{ARRAY_OFFSET}",          // 1 cycle
    // check if ARRAY_OFFSET == ARRAY1
    "cmp r1, r2", // r2 contains the current array_offset
    "bne .OFFSET_ARRAY2",
    // base == arry1
    "movw r1, :lower16:{ARRAY2}",
    "movt r1, :upper16:{ARRAY2}",
    // store mem adress of array2 in array_offset
    "str r4, [r2]",
    // return out of the interrupt
    "bx lr",                                     // 1 cycle minimum

    ".OFFSET_ARRAY2:",
    // set ARRAY_OFFSET to ARRAY_BASE
    "movw r1, :lower16:{ARRAY1}",
    "movt r1, :upper16:{ARRAY1}",
    // store mem adress of array1 in array_offset
    "str r1, [r3]",
    // return out of the interrupt
    "bx lr",


    ".cfi_endproc",
    ".cantunwind",
    ".fnend",

    ARRAY1 = sym ARRAY1,
    ARRAY2 = sym ARRAY2,
    ARRAY_OFFSET = sym ARRAY_OFFSET,
    DONE = sym DONE,
    // GPIO_STATE_PTR = sym GPIO_STATE_PTR,
}

pub static mut ARRAY1: *const u32 = unsafe { ARRAY.as_ptr() };
pub static mut ARRAY2: *const u32 = unsafe { ARRAY.as_ptr().add(ARRAY_LEN / 2) };
pub static mut ARRAY_OFFSET: *const u32 = unsafe { ARRAY1 };
