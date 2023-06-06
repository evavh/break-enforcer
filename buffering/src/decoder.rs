pub trait DataHandler<const LEN: usize> {
    // try to read the data, this can be pre-empted by an
    // interrupt. Do not process the data before its marked
    // complete
    fn attempt(&mut self, data: &[u32; LEN]);
    // last read was done before another interrupt fired
    // it is not corrupt and can be processed
    fn mark_success(&mut self);
    fn mark_corrupt(&mut self);
    // could not read fast enough, missed n packages
    fn lost(&mut self, n: usize);
}

pub struct CopyDecoder<const LEN: usize> {
    pub buffer: [u32; LEN],
    pub done: bool, // default false
    pub lost: usize,
}

impl<const LEN: usize> Default for CopyDecoder<LEN> {
    fn default() -> Self {
        Self {
            buffer: [0u32; LEN],
            done: false,
            lost: 0,
        }
    }
}

impl<const LEN: usize> DataHandler<LEN> for CopyDecoder<LEN> {
    fn attempt(&mut self, data: &[u32; LEN]) {
        self.buffer = *data
    }

    fn mark_success(&mut self) {
        self.done = true
    }
    fn mark_corrupt(&mut self) {}

    fn lost(&mut self, n: usize) {
        self.lost += n
    }
}
