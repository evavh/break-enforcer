#![feature(const_option)]
#![feature(concat_idents)]

mod irq_test;
mod reader;

use irq_test::FakeIrqList;
use reader::Reader;

use std::sync::atomic::{AtomicUsize, Ordering};

const BUFFER_DEPTH: usize = 4;
const ARRAY_LEN: usize = 2;
// *2 as there are two 'arrays' between which we alternate
static mut ARRAY_STORE: [[u32; ARRAY_LEN]; BUFFER_DEPTH] = [[5u32; ARRAY_LEN]; BUFFER_DEPTH];
static mut ARRAY_FIRST: *const u32 = unsafe { ARRAY_STORE.first().unwrap().as_ptr() };
static mut ARRAY_LAST: *const u32 = unsafe { ARRAY_STORE.last().unwrap().as_ptr() };
static mut ARRAY_OFFSET: *const u32 = unsafe { ARRAY_FIRST };

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// read_next needs to be init to 0 first time this is called
pub fn read(mut fake_irq: impl FakeIrqList, read_upto: &mut usize, reader: &mut impl Reader) {
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
    if wrote_upto > *read_upto + BUFFER_DEPTH {
        reader.lost(wrote_upto - *read_upto - BUFFER_DEPTH);
        *read_upto = wrote_upto - BUFFER_DEPTH;
    }

    let idx = *read_upto % BUFFER_DEPTH;
    reader.attempt(idx);
    fake_irq.after_read();
    let after_read = NEXT.load(Ordering::Relaxed);
    if after_read - wrote_upto < BUFFER_DEPTH {
        reader.mark_success();
        *read_upto += 1;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::sync::Mutex;

    use super::*;
    use irq_test::simulate_irq;

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
        use reader::CopyReader;

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
        fn missed_before_first_read() {
            let _lock = MUTEX.lock();
            unsafe {
                // mutex prevents parallel access
                init_statics();

                // lose one packet
                for i in 0..(BUFFER_DEPTH + 1) {
                    simulate_irq(1 + i as u32);
                }
                let mut reader = CopyReader::default();

                let mut read_upto = 0;
                read((), &mut read_upto, &mut reader);
                assert_eq!(reader.buffer, [2; ARRAY_LEN]);
                read((), &mut read_upto, &mut reader);
                assert_eq!(reader.buffer, [3; ARRAY_LEN]);
                read((), &mut read_upto, &mut reader);
                assert_eq!(reader.buffer, [4; ARRAY_LEN]);
                read((), &mut read_upto, &mut reader);
                assert_eq!(reader.buffer, [5; ARRAY_LEN]);
            };
        }

        #[test]
        fn missed_in_the_middle() {
            let _lock = MUTEX.lock();
            unsafe {
                // mutex prevents parallel access
                init_statics();

                // lose one packet
                let mut numb = 1;
                for _ in 0..(BUFFER_DEPTH) {
                    simulate_irq(numb);
                    numb += 1;
                }
                let mut reader = CopyReader::default();

                let mut read_upto = 0;
                read((), &mut read_upto, &mut reader);
                assert_eq!(reader.buffer, [1; ARRAY_LEN]);
                simulate_irq(numb);
                numb += 1;
                simulate_irq(numb);
                read((), &mut read_upto, &mut reader);
                assert_eq!(reader.lost, 1);
                assert_eq!(reader.buffer, [3; ARRAY_LEN]);
                read((), &mut read_upto, &mut reader);
                assert_eq!(reader.buffer, [4; ARRAY_LEN]);
                read((), &mut read_upto, &mut reader);
                assert_eq!(reader.buffer, [5; ARRAY_LEN]);
                read((), &mut read_upto, &mut reader);
                assert_eq!(reader.buffer, [6; ARRAY_LEN]);
            };
        }
    }
    mod irq_inbetween {
        use super::*;
        use irq_test::Simulated;
        use reader::CopyReader;

        #[test]
        fn read_one() {
            let _lock = MUTEX.lock();
            unsafe {
                // mutex prevents parallel access
                init_statics();

                let simulated = Simulated {
                    irq_numb: 43,
                    n_in_while: 1,
                    n_after_while: 1,
                    n_after_read: 1,
                };
                simulate_irq(42);
                let mut reader = CopyReader::default();
                read(simulated, &mut 0, &mut reader);
                assert_eq!(reader.buffer, [42; ARRAY_LEN]);
            }
        }

        #[test]
        fn not_corrupt_when_not_overwriting() {
            let _lock = MUTEX.lock();
            unsafe {
                // mutex prevents parallel access
                init_statics();

                let simulated = Simulated {
                    n_after_read: 3,
                    ..Default::default()
                };
                simulate_irq(42);
                let mut reader = CopyReader::default();
                read(simulated, &mut 0, &mut reader);

                assert_eq!(reader.done, true);
            }
        }

        #[test]
        fn corrupt_when_overwriting() {
            let _lock = MUTEX.lock();
            unsafe {
                // mutex prevents parallel access
                init_statics();

                let simulated = Simulated {
                    n_after_read: 4,
                    ..Default::default()
                };
                simulate_irq(42);
                let mut reader = CopyReader::default();
                read(simulated, &mut 0, &mut reader);

                assert_eq!(reader.done, false);
            }
        }
    }
}
