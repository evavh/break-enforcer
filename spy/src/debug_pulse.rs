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

pub const DEBUG_GPIO_REG: u32 = 0x40020814;
// debug pin (pa0) is only pin in GPIOA (except clock pin which ignores this)
pub const DEBUG_ON: u32 = 1;
pub const DEBUG_OFF: u32 = 0;

// 100 nops should take about 1.2 usecs and they do
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
    "movs r0, #20",                                 // 1 cycle
    "movs r1, #1",                                  // 1 cycle 
    "movt r0, #16386",                              // 1 cycle
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
