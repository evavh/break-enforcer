use crate::{ARRAY_BYTES, ARRAY_STORE, NEXT};
use core::arch::global_asm;

static mut ARRAY_OFFSET: *const u8 = unsafe { ARRAY_STORE.first().unwrap().as_ptr() };
// these are created in the linker script. Their adress
// points to the first and last array in ARRAY_STORE
// this allows us to use only mov and not load anything 
// saving 4 cycles. 
//
// we need to do this via the linker as we can not pass pointer
// values to the assembly macro.
extern "C" {
    pub static ARRAY_FIRST: [u8; 1];
    pub static ARRAY_LAST: [u8; 1];
}

// Note: only use R0,R1,R2,R3 and R12. Others are not saved by the
// mcu before entering the interrupt.
global_asm! {
    ".section .data.EXTI1",
    ".global EXTI1",
    ".p2align 4",
    ".type EXTI1,%function",
    ".thumb_func",
"EXTI1:",
    /*
    // output debug pulse
    "movs r0, #20",
    "movs r1, #1",                                  // 1 cycle
    "movt r0, #16386",                              // 1 cycle
    "str r1, [r0]",                                 // 2 cycles
    "movs r1, #0",                                  // 1 cycle
    "str r1, [r0]",                                 // 2 cycles
                                                    // = 8 cycles
    // we see the pulse 292 ns after interrupt should have been triggerd
    // 84 Mhz = 11.9 ns per cycle
    // 25.5 cycles after start of interrupt
    // 1 usb pulse (half wave) at 12 Mhz is 83 nsec or 7 cycles
    // after the first str we are at 3.6 USB cycles
    // we want to be in the middle of a usb cycle when we start reading
    */

    // 12 cycles past interrupt source

    // Set the pin state adress in r0
    "movw r0, #1040",
    "movt r0, #16386", // TODO get this in here as a const?
    // add nops so we read in the center of the usb clock
    "NOP", // 1 cycle
    // reads the pin state ASAP
    // this is cycle 17 past interrupt source
    // that means 17/7 ~= 2.5
    "ldr r1, [r0]",                              // 2 cycles
    "NOP",

    // build the pointer to ARRAY in r2 so we can store it
    // it should be the value at memory adress ARRAY_OFFSET
    "movw r3, :lower16:{ARRAY_OFFSET}",          // 1 cycle
    "movt r3, :upper16:{ARRAY_OFFSET}",          // 1 cycle
    "ldr r2, [r3]",                              // 2 cycles
    // = 7 cycles after first read



    // store pin states in ARRAY[1] and ARRAY[2]
    // and prepare to set interrupt pending to false
    "ldr r3, [r0]",                              // 2 cycles
    // we start at index 4 as the 0th 32 bits are used
    // for the length of the packet
    // see beyond label: `EXIT_READ_PACKETS`
    "strb r1, [r2, #4]",                          // 2 cycles
    "strb r3, [r2, #5]",                          // 2 cycles
    "NOP",
    // = 14 cycles after first read



    // store pin state in ARRAY[3]
    // start increment NEXT static
    "ldr r1, [r0]",                              // 2 cycles
    "strb r1, [r2, #6]",                         // 2 cycles
    "movw r3, :lower16:{NEXT}",                  // 1 cycle
    "movt r3, :upper16:{NEXT}",                  // 1 cycle
    "NOP",                                       // 1 cycle
    // // = 21 cycles after first read



    // store pin state in ARRAY[4]
    // continue incrementing NEXT static
    "ldr r1, [r0]",                              // 2 cycles
    "strb r1, [r2, #7]",                         // 2 cycles
    // load NEXT and add 1
    "ldr r12, [r3]",                             // 2 cycles
    "ADD r12, r12, #1",                          // 1 cycle
    // = n*7 cycles after first read



    // store pin state in ARRAY[5]
    // continue incrementing NEXT static
    "ldr r1, [r0]",                              // 2 cycles
    "strb r1, [r2, #8]",                         // 2 cycles
    // commit NEXT to memory
    "str r12, [r3]",                             // 2 cycles
    // set length to 4
    "movw r3, #5",                               // 1 cycle
    // = n*7 cycles after first read


    // store pin state in ARRAY[6]
    // start mark interrupt as no longer pending
    "ldr r1, [r0]",                              // 2 cycles
    "strb r1, [r2, #9]",                         // 2 cycles
    "movw r12, #15380",                          // 1 cycle
    "movt r12, #16385",                          // 1 cycle
    "NOP",                                       // 1 cycle
    // = n*7 cycles after first read
    // only r12 is free after here

    // // debug pulse surrounding ARRAY values
    // // WARNING: will fuck up index
    // "movs r12, #20",
    // "movt r12, #16386",
   //  // "movs r3, #1",
   //  // "str r3, [r12]",                                 // 2 cycles (debug pin high)


    // Store gpio value in ARRAY (repeat N-4 times)
    // Because:
    //  - the first read is combined with setting up the array pointer
    //  - the second+third read is combined with setting the interrupt as handled
    //  - the fourth+fifth read sets the new data bool to true
    include_str!(concat!(env!("OUT_DIR"), "/loop.s")), // should be N * 7
    ".EXIT_READ_PACKETS:",


    // // debug pulse end
    // "movs r3, #0",                                   // 1 cycle
    // "str r3, [r12]",                                 // 2 cycles (debug pin low)

    // mark interrupt as handled
    "movs r1, #2",                               // 1 cycle
    "str r1, [r12]",                             // 2 cycles
    // store length of packet(r3) in array[0]
    "str r3, [r2, #0]",                          // 2 cycles

    // add array_len (smaller then 4095) to curr(r2)
    "ADD r2, r2, #{ARRAY_BYTES}",
    // load array_last to r3
    "movw r3, :lower16:{ARRAY_LAST}",
    "movt r3, :upper16:{ARRAY_LAST}",

    // check if curr(r2) is larger then array_last(r3)
    "cmp r3, r2", // subtracts curr(r2) from array_last(r3) set flag if res negative

    // If Then Then, next 2 commands are only executed if
    // the minus flag is set
    "ITT MI",                                       // 0 cycles, folded into cmp above
    // if curr > array_last do wrap around and set curr = array_first
    "movwmi r2, :lower16:{ARRAY_FIRST}",            // 1 cycle
    "movtmi r2, :upper16:{ARRAY_FIRST}",            // 1 cycle

    // commit curr to ram
    "movw r3, :lower16:{ARRAY_OFFSET}",             // 1 cycle
    "movt r3, :upper16:{ARRAY_OFFSET}",             // 1 cycle
    "str r2, [r3]",

    // // debug pulse
    // "movs r0, #20",
    // "movt r0, #16386",                              // 1 cycle
    // "movs r1, #1",                                  // 1 cycle
    // "str r1, [r0]",                                 // 2 cycles
    // "movs r1, #0",                                  // 1 cycle
    // "str r1, [r0]",                                 // 2 cycles
    //                                                 // = 7 cycles

    // return out of interrupt
    "bx lr",                                        // 1 + P cycles

    ARRAY_OFFSET = sym ARRAY_OFFSET,
    ARRAY_FIRST = sym ARRAY_FIRST,
    ARRAY_LAST = sym ARRAY_LAST,
    ARRAY_BYTES = const ARRAY_BYTES,
    NEXT = sym NEXT,
}
