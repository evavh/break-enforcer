use core::hint::unreachable_unchecked;

#[inline]
pub(super) fn mask<const P: u16>(register: u32) -> Sample {
    // shift the bit representing the pin of intrested to position 0.
    let port = register >> P;
    // make everything else 1 becomes zero
    let port = port & 1;
    match port {
        0 => Sample::Low,
        1 => Sample::High,
        _ => unsafe { unreachable_unchecked() },
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Sample {
    High = 1,
    Low = 0,
}

impl Sample {
    pub(super) fn char(&self) -> char {
        match self {
            Sample::High => '1',
            Sample::Low => '0',
        }
    }

    pub(super) fn from_bit(bit: u32) -> Self {
        if bit == 0 {
            Sample::Low
        } else {
            Sample::High
        }
    }
}
