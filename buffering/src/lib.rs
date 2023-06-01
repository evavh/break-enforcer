#![feature(const_option)]
#![feature(concat_idents)]

use std::sync::atomic::{AtomicUsize, Ordering};

const BUFFER_DEPTH: usize = 4;
const ARRAY_LEN: usize = 2;
// *2 as there are two 'arrays' between which we alternate
static mut ARRAY_STORE: [[u32; ARRAY_LEN]; BUFFER_DEPTH] = [[5u32; ARRAY_LEN]; BUFFER_DEPTH];
static mut ARRAY_FIRST: *const u32 = unsafe { ARRAY_STORE.first().unwrap().as_ptr() };
static mut ARRAY_LAST: *const u32 = unsafe { ARRAY_STORE.last().unwrap().as_ptr() };
static mut ARRAY_OFFSET: *const u32 = unsafe { ARRAY_FIRST };

static NEXT: AtomicUsize = AtomicUsize::new(0);

trait FakeIrqList {
    fn in_while(&mut self) {}
    fn after_while(&mut self) {}
    fn after_read(&mut self) {}
}

impl FakeIrqList for () {}

struct Simulated {
    irq_numb: usize,
    n_in_while: usize,
    n_after_while: usize,
    n_after_read: usize,
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

trait Reader {
    // try to read the data, this can be pre-empted by an
    // interrupt. Do not process the data before its marked
    // complete
    fn attempt(&mut self, idx: usize);
    // last read was done before another interrupt fired
    // it is not corrupt and can be processed
    fn mark_success(&mut self);
    // could not read fast enough, missed n packages
    fn lost(&mut self, n: usize);
}

#[derive(Default)]
struct CopyReader {
    buffer: [u32; ARRAY_LEN],
    done: bool, // default false
}

impl Reader for CopyReader {
    fn attempt(&mut self, idx: usize) {
        self.buffer = unsafe { ARRAY_STORE[dbg!(idx)] }
    }

    fn mark_success(&mut self) {
        self.done = true
    }

    fn lost(&mut self, n: usize) {
        todo!()
    }
}

/// read_next needs to be init to 0 first time this is called
fn read(mut fake_irq: impl FakeIrqList, read_upto: &mut usize, reader: &mut impl Reader) {
    // overflow/wrap around can be ignored as we expect
    // a reboot before 40 days of activity
    // (assuming 1000 messages per second)
    let mut wrote_upto = NEXT.load(Ordering::Relaxed);
    while *read_upto == wrote_upto {
        fake_irq.in_while();
        wrote_upto = NEXT.load(Ordering::Relaxed);
    }
    fake_irq.after_while();

    // wrote_upto = 0
    // BUFFER_DEPTH = 4
    // read idx: 0 mod 4 = 1
    let idx = *read_upto % BUFFER_DEPTH;
    reader.attempt(idx);
    fake_irq.after_read();
    let after_read = NEXT.load(Ordering::Relaxed);
    if after_read - wrote_upto < 4 {
        reader.mark_success();
        *read_upto += 1;
    } else {
        reader.lost(after_read - 4);
        *read_upto = wrote_upto - 4;
    }
}

/// do not run in parallel
unsafe fn simulate_irq(numb: u32) {
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::sync::Mutex;

    use super::*;

    unsafe fn init_statics() {
        ARRAY_OFFSET = ARRAY_FIRST;
        ARRAY_STORE = [[5u32; ARRAY_LEN]; BUFFER_DEPTH];
        NEXT.store(0, Ordering::Relaxed);
    }

    // do not run tests in parallel
    static MUTEX: Mutex<()> = Mutex::new(());

    mod simulated_irq {
        use super::*;

        #[test]
        fn numb_increases() {
            let _lock = MUTEX.lock();
            unsafe {
                // mutex prevents parallel access
                init_statics();

                simulate_irq(0);
                simulate_irq(0);
                assert_eq!(NEXT.load(Ordering::Relaxed), 2);
            }
        }
    }

    mod no_irq_inbetween {
        use super::*;

        #[test]
        fn read_one() {
            let _lock = MUTEX.lock();
            unsafe {
                // mutex prevents parallel access
                init_statics();

                simulate_irq(42);
                dbg!(ARRAY_STORE);
                let mut reader = CopyReader::default();
                read((), &mut 0, &mut reader);

                assert_eq!(reader.buffer, [42; ARRAY_LEN]);
            };
        }

        #[test]
        fn missed_one() {
            let _lock = MUTEX.lock();
            unsafe {
                // mutex prevents parallel access
                init_statics();

                // lose one packet
                for i in 0..(BUFFER_DEPTH + 1) {
                    simulate_irq(1 + i as u32);
                }
                dbg!(ARRAY_STORE);
                let mut reader = CopyReader::default();

                read((), &mut 0, &mut reader);
                assert_eq!(reader.buffer, [2; ARRAY_LEN]);
                read((), &mut 0, &mut reader);
                assert_eq!(reader.buffer, [3; ARRAY_LEN]);
                read((), &mut 0, &mut reader);
                assert_eq!(reader.buffer, [4; ARRAY_LEN]);
                read((), &mut 0, &mut reader);
                assert_eq!(reader.buffer, [5; ARRAY_LEN]);
            };
        }
    }
    #[test]
    fn reads_in_order() {}
}
