use core::hash::Hasher;

use crate::decoder::Sample;

/// compact representation of a bit list that
/// can easily be hashed
pub struct Packet<const LEN: usize>
where
    [(); (LEN + 31) / 32]:,
{
    buf: [u32; (LEN + 31) / 32], // if only we had div ceil...
    bits: u16,
}

impl<const LEN: usize> Packet<LEN>
where
    [(); (LEN + 31) / 32]:,
{
    pub(super) const fn new() -> Self {
        Self {
            buf: [0u32; (LEN + 31) / 32],
            bits: 0,
        }
    }

    pub(super) fn reset(&mut self) {
        self.bits = 0;
    }

    pub(super) fn get(&self, idx: u16) -> Sample {
        let byte_idx = idx / 32;
        let bit_idx = idx % 32;
        let word = self.buf[byte_idx as usize];
        let bit = (word >> bit_idx) & 1;
        Sample::from_bit(bit)
    }

    pub(super) fn push(&mut self, val: Sample) {
        let byte_idx = self.bits / 32;
        let bit_idx = self.bits % 32;
        let mask = (val as u32) << bit_idx;
        self.buf[byte_idx as usize] |= mask;
        self.bits += 1;
    }

    pub(crate) fn hash(&self) -> usize {
        let mut hash = rustc_hash::FxHasher::default();
        for word in self.buf {
            hash.write_u32(word);
        }
        // on 32 bit platforms this is a 32 bit hasher
        hash.finish() as usize
    }
}

pub struct PacketIterator<'a, const LEN: usize>
where
    [(); (LEN + 31) / 32]:,
{
    packet: &'a Packet<LEN>,
    next_bit: u16,
}

impl<'a, const LEN: usize> IntoIterator for &'a Packet<LEN>
where
    [(); (LEN + 31) / 32]:,
{
    type Item = Sample;
    type IntoIter = PacketIterator<'a, LEN>;

    fn into_iter(self) -> Self::IntoIter {
        PacketIterator {
            packet: self,
            next_bit: 0,
        }
    }
}

impl<'a, const LEN: usize> Iterator for PacketIterator<'a, LEN>
where
    [(); (LEN + 31) / 32]:,
{
    type Item = Sample;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next_bit >= self.packet.bits {
            return None;
        }

        let sample = self.packet.get(self.next_bit);
        self.next_bit += 1;
        Some(sample)
    }
}

impl<const LEN: usize> defmt::Format for Packet<LEN>
where
    [(); (LEN + 31) / 32]:,
{
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "\n[");
        for sample in self {
            defmt::write!(fmt, "{}", sample.char());
        }
        defmt::write!(fmt, "]\n");
    }
}
