#![cfg_attr(feature = "nostd", no_std)]
#![feature(const_option)]
#![feature(concat_idents)]

#[cfg(not(feature = "nostd"))]
#[cfg(test)]
mod irq_test;

pub trait FakeIrqList {
    fn in_while(&mut self) {}
    fn after_while(&mut self) {}
    fn after_read(&mut self) {}
}
impl FakeIrqList for () {}

mod decoder;
pub use decoder::DataHandler;

use core::sync::atomic::{AtomicUsize, Ordering};

pub struct SwapBufReader<'a, const SIZE: usize, const DEPTH: usize> {
    pub raw: &'a [[u8; SIZE]; DEPTH],
    pub next: &'a AtomicUsize,
}

impl<'a, const SIZE: usize, const DEPTH: usize> SwapBufReader<'a, SIZE, DEPTH> {
    /// read_next needs to be init to 0 first time this is called
    /// note LEN =< SIZE
    pub fn read(
        &self,
        mut fake_irq: impl FakeIrqList,
        read_upto: &mut usize,
        reader: &mut impl DataHandler<SIZE>,
    ) {
        // overflow/wrap around can be ignored as we expect
        // a reboot before 40 days of activity
        // (assuming 1000 messages per second)
        let mut wrote_upto = self.next.load(Ordering::Relaxed);
        while *read_upto == wrote_upto {
            fake_irq.in_while();
            wrote_upto = self.next.load(Ordering::Relaxed);
        }
        fake_irq.after_while();

        // wrote_upto = 0
        // BUFFER_DEPTH = 4
        // read idx: 0 mod 4 = 1
        if wrote_upto > *read_upto + DEPTH {
            reader.lost(wrote_upto - *read_upto - DEPTH);
            *read_upto = wrote_upto - DEPTH;
        }

        let idx = *read_upto % DEPTH;
        let unread = self.raw[idx];
        reader.attempt(&unread);
        fake_irq.after_read();
        let after_read = self.next.load(Ordering::Relaxed);
        if after_read - wrote_upto < DEPTH {
            reader.mark_success();
            *read_upto += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use irq_test::simulate_irq;

    mod no_irq_inbetween {
        use core::time::Duration;
        use std::thread;

        use crate::irq_test::Buffer;

        use super::*;
        use decoder::CopyDecoder;

        #[test]
        fn read_zero() {
            let mut decoder: CopyDecoder<5> = CopyDecoder::default();
            let buffer: Buffer<5, 4> = Buffer::default();

            // assert this hangs if no interrupts are made
            let handle = thread::spawn(move || {
                let buffer_ref = SwapBufReader {
                    raw: &buffer.store,
                    next: &buffer.next,
                };
                buffer_ref.read((), &mut 0, &mut decoder);
            });

            thread::sleep(Duration::from_millis(100));
            assert!(!handle.is_finished())
        }

        #[test]
        fn read_one() {
            let mut decoder: CopyDecoder<5> = CopyDecoder::default();
            let mut buffer: Buffer<5, 4> = Buffer::default();

            simulate_irq(42, &mut buffer);
            let buffer_ref = SwapBufReader {
                raw: &buffer.store,
                next: &buffer.next,
            };
            buffer_ref.read((), &mut 0, &mut decoder);

            assert_eq!(decoder.buffer.as_slice(), &vec![42; buffer.len()]);
        }

        #[test]
        fn missed_before_first_read() {
            let mut buffer: Buffer<5, 4> = Buffer::default();
            // lose one packet
            for i in 0..(buffer.depth() + 1) {
                simulate_irq(1 + i as u32, &mut buffer);
            }
            let mut decoder = CopyDecoder::default();
            let buf_reader = buffer.reader();

            let mut read_upto = 0;
            buf_reader.read((), &mut read_upto, &mut decoder);
            assert_eq!(decoder.lost, 1);
            assert_eq!(decoder.buffer.as_slice(), &vec![2; buffer.len()]);
            buf_reader.read((), &mut read_upto, &mut decoder);
            assert_eq!(decoder.buffer.as_slice(), &vec![3; buffer.len()]);
            buf_reader.read((), &mut read_upto, &mut decoder);
            assert_eq!(decoder.buffer.as_slice(), &vec![4; buffer.len()]);
            buf_reader.read((), &mut read_upto, &mut decoder);
            assert_eq!(decoder.buffer.as_slice(), &vec![5; buffer.len()]);
        }

        #[test]
        fn missed_in_the_middle() {
            let mut buffer: Buffer<5, 4> = Buffer::default();
            // lose one packet
            let mut numb = 1;
            for _ in 0..(buffer.depth()) {
                simulate_irq(numb, &mut buffer);
                numb += 1;
            }
            let mut decoder = CopyDecoder::default();
            let mut read_upto = 0;
            buffer.reader().read((), &mut read_upto, &mut decoder);
            assert_eq!(decoder.buffer.as_slice(), &vec![1; buffer.len()]);
            simulate_irq(numb, &mut buffer);
            numb += 1;
            simulate_irq(numb, &mut buffer);
            buffer.reader().read((), &mut read_upto, &mut decoder);
            assert_eq!(decoder.lost, 1);
            assert_eq!(decoder.buffer.as_slice(), &vec![3; buffer.len()]);
            buffer.reader().read((), &mut read_upto, &mut decoder);
            assert_eq!(decoder.buffer.as_slice(), &vec![4; buffer.len()]);
            buffer.reader().read((), &mut read_upto, &mut decoder);
            assert_eq!(decoder.buffer.as_slice(), &vec![5; buffer.len()]);
            buffer.reader().read((), &mut read_upto, &mut decoder);
            assert_eq!(decoder.buffer.as_slice(), &vec![6; buffer.len()]);
        }

        #[test]
        fn lose_a_lot() {
            let mut buffer: Buffer<5, 4> = Buffer::default();
            // lose one packet
            let mut numb = 1;
            for _ in 0..(buffer.depth() + 10_000) {
                simulate_irq(numb, &mut buffer);
                numb += 1;
            }
            let mut decoder = CopyDecoder::default();
            let mut read_upto = 0;
            buffer.reader().read((), &mut read_upto, &mut decoder);
            assert_eq!(decoder.buffer.as_slice(), &vec![10_001u32; buffer.len()]);
            assert_eq!(decoder.lost, 10_000);
        }
    }
    mod irq_inbetween {
        use super::*;
        use decoder::CopyDecoder;
        use irq_test::{Buffer, Simulated};

        unsafe fn steal<'a, T>(reference: &T) -> &'static mut T {
            let const_ptr = reference as *const T;
            let mut_ptr = const_ptr as *mut T;
            let mut_ref = &mut *mut_ptr;
            std::mem::transmute::<&'a mut T, &'static mut T>(mut_ref)
        }

        #[test]
        fn read_one() {
            let buffer: Buffer<5, 4> = Buffer::default();
            let mut simulated = Simulated {
                irq_numb: 43,
                n_in_while: 1,
                n_after_while: 1,
                n_after_read: 1,
                data: unsafe { steal(&buffer) },
            };
            simulate_irq(42, &mut simulated.data);
            let mut decoder = CopyDecoder::default();

            buffer.reader().read(simulated, &mut 0, &mut decoder);
            assert_eq!(decoder.buffer.as_slice(), &vec![42; buffer.len()]);
        }

        #[test]
        fn not_corrupt_when_not_overwriting() {
            let buffer: Buffer<5, 4> = Buffer::default();
            let mut simulated = Simulated {
                n_after_read: 3,
                data: unsafe { steal(&buffer) },
                irq_numb: 0,
                n_in_while: 0,
                n_after_while: 0,
            };
            simulate_irq(42, &mut simulated.data);
            let mut decoder = CopyDecoder::default();
            let buf_reader = buffer.reader();

            buf_reader.read(simulated, &mut 0, &mut decoder);

            assert_eq!(decoder.done, true);
        }

        #[test]
        fn corrupt_when_overwriting() {
            let mut buffer: Buffer<5, 4> = Buffer::default();
            simulate_irq(42, &mut buffer);
            let buf_reader = buffer.reader();

            let simulated = Simulated {
                n_after_read: 4,
                data: unsafe { steal(&buffer) },
                irq_numb: 0,
                n_in_while: 0,
                n_after_while: 0,
            };
            let mut decoder = CopyDecoder::default();
            buf_reader.read(simulated, &mut 0, &mut decoder);

            assert_eq!(decoder.done, false);
        }
    }
}
