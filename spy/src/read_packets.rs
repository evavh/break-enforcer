use core::arch::global_asm;
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

global_asm! {
    ".section .data.EXTI1",
    ".global EXTI1",
    ".p2align 1",
    ".type EXTI1,%function",
    ".code 16",
    ".thumb_func",
"EXTI1:",
    ".fnstart",
    // ".cfi_startproc",
    ".save {{r7, lr}}",
    "push {{r7, lr}}",
	// ".cfi_def_cfa_offset 8",
	// ".cfi_offset lr, -4",
	// ".cfi_offset r7, -8",
	".setfp	r7, sp",
    "mov r7, sp",
    // ".cfi_def_cfa_register r7",

    // Prepare registers
    //
    // pin adress in r0
    "movw r0, #1040",                             // 1 cycle
    // keep start of ARRAY in r2
    "movw r2, :lower16:{ARRAY}",         // 1 cycle
    // continue moving pin addr to r0
    "movt r0, #16386",                            // 1 cycle
    // continue putting ARRAY in r2
    "movt r2, :upper16:{ARRAY}",         // 1 cycle


    // Store gpio value in ARRAY
    //
    // load the current value of the pin into r1
    "ldr r1, [r0]",                               // 2 cycles
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
    "movw r0, :lower16:{DONE}",         // 1 cycle
    "movt r0, :upper16:{DONE}",         // 1 cycle
    // set r1 to the pr1 bit (1)
    "mov r1, #1",                                // 1 cycle
    // set DONE to r1 (which is 1)
    "strb r1, [r0]",                             // 2 cycles
    
    "pop {{r7, pc}}",
    
	// ".size	EXTI1, .Lfunc_end2-EXTI1",
	// ".cfi_endproc",
	".cantunwind",
	".fnend",

    ARRAY = sym ARRAY,
    DONE = sym DONE,
}
