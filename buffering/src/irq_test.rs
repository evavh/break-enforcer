use core::sync::atomic::{AtomicUsize, Ordering};

use crate::{SwapBufReader, FakeIrqList};

pub(super) struct Simulated<const LEN: usize, const DEPTH: usize> {
    pub irq_numb: usize,
    pub n_in_while: usize,
    pub n_after_while: usize,
    pub n_after_read: usize,

    pub data: &'static mut Buffer<LEN, DEPTH>,
}

macro_rules! simulate {
    ($name:ident, $var:ident) => {
        fn $name(&mut self) {
            for _ in 0..self.$var {
                self.irq_numb += 1;
                simulate_irq(self.irq_numb as u32, &mut self.data);
            }
        }
    };
}

impl<const LEN: usize, const DEPTH: usize> FakeIrqList for Simulated<LEN, DEPTH> {
    simulate!(in_while, n_in_while);
    simulate!(after_while, n_after_while);
    simulate!(after_read, n_after_read);
}

pub(super) struct Buffer<const LEN: usize, const DEPTH: usize> {
    pub store: [[u32; LEN]; DEPTH],
    pub next: AtomicUsize,
    offset: *const u32,
}

impl<const LEN: usize, const DEPTH: usize> Default for Buffer<LEN, DEPTH> {
    fn default() -> Self {
        let mut buf = Self {
            store: [[5u32; LEN]; DEPTH],
            next: AtomicUsize::new(0),
            offset: 0 as *const u32,
        };
        buf.offset = buf.first();
        buf
    }
}

impl<const LEN: usize, const DEPTH: usize> Buffer<LEN, DEPTH> {
    fn first(&self) -> *const u32 {
        self.store.first().unwrap().as_ptr()
    }
    fn last(&self) -> *const u32 {
        self.store.last().unwrap().as_ptr()
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        LEN
    }
    #[cfg(test)]
    pub(super) fn depth(&self) -> usize {
        DEPTH
    }
    #[cfg(test)]
    pub(super) fn reader<'a>(&'a self) -> SwapBufReader<'a, LEN, DEPTH> {
        SwapBufReader {
            raw: &self.store,
            next: &self.next,
        }
    }
}

/// do not run in parallel
pub(super) fn simulate_irq<const LEN: usize, const DEPTH: usize>(
    numb: u32,
    data: &mut Buffer<LEN, DEPTH>,
) {
    #[cfg(not(feature = "nostd"))]
    eprintln!("simulating irq: {}", numb);
    use core::ptr::slice_from_raw_parts_mut;

    let array_offset = data.offset as *mut u32;
    let curr = slice_from_raw_parts_mut(array_offset, LEN);
    unsafe {
        curr.as_mut().unwrap().clone_from_slice(&[numb; LEN]);
    }

    unsafe {
        data.offset = data.offset.add(LEN);
    }
    if data.offset > data.last() {
        data.offset = data.first();
    }
    data.next.fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let mut buffer: Buffer<2, 4> = Buffer::default();
        simulate_irq(0, &mut buffer);
        simulate_irq(0, &mut buffer);
        assert_eq!(buffer.next.load(Ordering::Relaxed), 2);
    }
}
