use core::arch::global_asm;

// https://developer.arm.com/documentation/100166/0001/Programmers-Model/Instruction-set-summary/Table-of-processor-instructions

// entire thing takes 6.37 usecs
// at 84 Mhz a NOP should take one clock cycle or ~= 12 ns
// 100 nops should take about 1.2 usecs. 
// This is at least a factor 5 off
//
// debug interrupt routine
global_asm! {
    ".section .data.EXTI1",
    ".global EXTI1",
    ".p2align 4",
    ".type EXTI1,%function",
    ".thumb_func",
"EXTI1:",
    ".fnstart",
    ".cfi_startproc",

    // start debug pulse
    // debug pin (pa0) is only pin in GPIOA (except clock pin which ignores this)
    // as it is in alternate mode
    "movs r0, #20",
    "movs r1, #1",
    "movt r0, #16386",
    "str r1, [r0]",                                 // 2 cycles
    
    // 100 nops
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",
    "NOP",

    // stop debug pulse
    "movs r1, #0",                                  // 1 cycle
    "str r1, [r0]",                                 // 2 cycles

    // mark interrupt as no longer pending
    "movw r3, #15380",                           // 1 cycle
    "movt r3, #16385",                           // 1 cycle
    "movs r12, #2",                               // 1 cycle
    "str r12, [r3]",                              // 2 cycles
    // return out of the interrupt
    "bx lr",

    ".cfi_endproc",
    ".cantunwind",
    ".fnend",
}
