use std::sync::atomic::Ordering;

use crate::{ARRAY_OFFSET, ARRAY_LEN, ARRAY_LAST, ARRAY_FIRST, NEXT};


pub trait FakeIrqList {
    fn in_while(&mut self) {}
    fn after_while(&mut self) {}
    fn after_read(&mut self) {}
}

impl FakeIrqList for () {}

#[derive(Default)]
pub struct Simulated {
    pub irq_numb: usize,
    pub n_in_while: usize,
    pub n_after_while: usize,
    pub n_after_read: usize,
}

macro_rules! simulate {
    ($name:ident, $var:ident) => {
        fn $name(&mut self) {
            for _ in 0..self.$var {
                self.irq_numb += 1;
                unsafe {
                    simulate_irq(self.irq_numb as u32);
                }
            }
        }
    };
}

impl FakeIrqList for Simulated {
    simulate!(in_while, n_in_while);
    simulate!(after_while, n_after_while);
    simulate!(after_read, n_after_read);
}

/// do not run in parallel
pub unsafe fn simulate_irq(numb: u32) {
    eprintln!("simulating irq: {}", numb); 
    use core::ptr::slice_from_raw_parts_mut;

    let array_offset = ARRAY_OFFSET as *mut u32;
    let curr = slice_from_raw_parts_mut(array_offset, ARRAY_LEN);
    (*curr)[0] = numb;
    (*curr)[1] = numb;

    ARRAY_OFFSET = ARRAY_OFFSET.add(ARRAY_LEN);
    if ARRAY_OFFSET > ARRAY_LAST {
        ARRAY_OFFSET = ARRAY_FIRST;
    }
    NEXT.fetch_add(1, Ordering::Relaxed);
}

