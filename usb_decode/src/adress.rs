use core::ops::Range;

use crate::{Crc5, Error};


pub struct Address {
    // address of usb device
    device: u8,
    // endpoint on device
    function: u8,
}

fn extract_byte(bits: &[u8], range: Range<usize>) -> u8 {
    let mut byte = 0;
    for (i, b) in bits[range].iter().enumerate() {
        byte |= b << i
    }

    byte
}

pub(crate) fn parse<C: Crc5>(mut bits: &[u8]) -> Result<(&[u8], Address), Error> {
    // todo need to decode this?
    let device = extract_byte(&mut bits, 0..7);
    let function = extract_byte(&mut bits, 7..7 + 4);
    let crc5 = extract_byte(&mut bits, 7 + 4..7 + 4 + 5);

    C::check(&bits[0..7 + 4], crc5).map_err(|_| "Crc mismatch")?;
    Ok((&bits[7 + 4 + 5..], Address { device, function }))
}

