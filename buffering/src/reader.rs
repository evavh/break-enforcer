use crate::{ARRAY_LEN, ARRAY_STORE};


pub trait Reader {
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
pub struct CopyReader {
    pub buffer: [u32; ARRAY_LEN],
    pub done: bool, // default false
    pub lost: usize,
}

impl Reader for CopyReader {
    fn attempt(&mut self, idx: usize) {
        self.buffer = unsafe { ARRAY_STORE[dbg!(idx)] }
    }

    fn mark_success(&mut self) {
        self.done = true
    }

    fn lost(&mut self, n: usize) {
        self.lost = n
    }
}
