use crate::{ARRAY2, ARRAY1, DONE};
use core::arch::global_asm;

pub const DEBUG_GPIO_REG: u32 = 0x40020814;
// debug pin (pa0) is only pin in GPIOA (except clock pin which ignores this)
pub const DEBUG_ON: u32 = 1;
pub const DEBUG_OFF: u32 = 0;

// Note: only use R0,R1,R2,R3 and R12. Others are not saved by the
// mcu before entering the interrupt.
global_asm! {
    ".section .data.EXTI1",
    ".global EXTI1",
    ".p2align 4",
    ".type EXTI1,%function",
    ".thumb_func",
"EXTI1:",
    ".fnstart",
    ".cfi_startproc",

    "/* {DEBUG_GPIO_REG} {DEBUG_ON} {DEBUG_OFF} */",
    // debug only, toggle debug pin
    // ".equ label, {DEBUG_GPIO_REG}",
    // "movw r0, #2068",
    // "movt r0, #16386",
    // "movw r1, #:lower16:{DEBUG_ON}",             // top 16 bit are zeroed
    // // "str r1, [r0]",                              // 2 cycles
    // "movw r1, #:lower16:{DEBUG_OFF}",            // top 16 bit are zeroed
    // // "str r1, [r0]",                              // 2 cycles
    
    // output debug pulse
    "movs r0, #20",
    "movs r1, #1",
    "movt r0, #16386",
    "str r1, [r0]",                                 // 2 cycles
    "movs r1, #0",                                  // 1 cycle
    "str r1, [r0]",                                 // 2 cycles
    // worst case 5 cycles high period should take 1/(1/84*5) = 16.8 Mhz
    // and we are measuring 8.... that is twice too slow...
    //
    // also note 5 clock cycles = 1 High speed clock pulse
    // thus the div on that clock is 5 (should be 10?)

    // 12 cycles past interrupt source

    // // Set the pin state adress in r0
    // "movw r0, #:lower16:{GPIO_STATE_PTR}",       // 1 cycle
    // "movt r0, #:upper16:{GPIO_STATE_PTR}",       // 1 cycle
    "movw r0, #1040",
    "movt r0, #16386", // TODO get this in here as a const?

    // add nops so we read in the center of the usb clock
    "NOP", // 1 cycle
    // reads the pin state ASAP
    // this is cycle 17 past interrupt source
    // that means 17/7 ~= 2.5
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
    // set 2nd (r12) irq_handled bit (at r3) to true
    // TODO: still needed? we do this at the end now
    // <16-05-23, dvdsk noreply@davidsk.dev>
    "movs r12, #2",                               // 1 cycle
    "str r12, [r3]",                              // 2 cycles
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
    // set DONE (r3) to 1 (r12)
    "mov r12, #1",                                // 1 cycle
    "strb r12, [r3]",                             // 2 cycles
    // = 35 cycles after first read


    // Store gpio value in ARRAY (repeat N-5 times)
    // Because:
    //  - the first read is combined with setting up the array pointer
    //  - the second+third read is combined with setting the interrupt as handled
    //  - the fourth+fifth read sets the new data bool to true


    // debug pulse surrounding ARRAY values
    "movs r12, #20",
    "movt r12, #16386",
    "movs r3, #1",
    // this pulse takes 154 micro seconds for 360 reads = 427 nsecs per cycle
    // thats 2.3 MHz while it should be 12. 
    // At 12Mhz each read should take 83.3 nsecs 
    // and the total should take 30 micro secs
    // this is 5 times too slow. It implies the cpu is running at 16.8 Mhz 
    "str r3, [r12]",                                 // 2 cycles (debug pin high)
    include_str!(concat!(env!("OUT_DIR"), "/loop.s")), // should be N * 7
    "movs r3, #0",                                   // 1 cycle
    "str r3, [r12]",                                 // 2 cycles (debug pin low)

    // // re-enable interrupts
    // "CPSIE",

    // mark interrupt as no longer pending
    "movw r3, #15380",                           // 1 cycle
    "movt r3, #16385",                           // 1 cycle
    "movs r12, #2",                               // 1 cycle
    "str r12, [r3]",                              // 2 cycles

    "movw r1, :lower16:{ARRAY1}",                // 1 cycle
    "movt r1, :upper16:{ARRAY1}",                // 1 cycle
    "movw r3, :lower16:{ARRAY_OFFSET}",          // 1 cycle
    "movt r3, :upper16:{ARRAY_OFFSET}",          // 1 cycle
    // check if ARRAY_OFFSET == ARRAY1
    "cmp r1, r2", // r2 contains the current array_offset
    "bne .OFFSET_ARRAY2",
    // base == arry1
    "movw r1, :lower16:{ARRAY2}",
    "movt r1, :upper16:{ARRAY2}",
    // store mem adress of array2 in array_offset
    "str r1, [r3]",
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
    DEBUG_GPIO_REG = const DEBUG_GPIO_REG,
    DEBUG_ON = const DEBUG_ON,
    DEBUG_OFF = const DEBUG_OFF,
}

pub static mut ARRAY_OFFSET: *const u32 = unsafe { ARRAY1.as_ptr() };
