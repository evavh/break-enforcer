use core::ops::Range;

// used to allow dependency injecting a hardware
// accellerated crc
pub trait Crc5 {
    // bits has no bit destuffing and is not NRZI decoded
    fn check(bits: &[u8], checksum: u8) -> Result<(), ()> {
        if Self::calc(bits) == checksum {
            Ok(())
        } else {
            Err(())
        }
    }
    fn calc(bits: &[u8]) -> u8;
}

// todo data packets have different crc?!?!
pub struct GenericCrc;
impl Crc5 for GenericCrc {
    fn calc(bits: &[u8]) -> u8 {
        use crc::{Crc, CRC_5_USB};
        let crc = Crc::<u8>::new(&CRC_5_USB);
        let mut digest = crc.digest();

        for byte in bits.chunks(8).map(|c| extract_byte(c, 0..c.len())) {
            dbg!(byte);
            digest.update(&[byte]);
        }
        // digest.update(bits);

        digest.finalize()
    }
}

fn extract_byte(bits: &[u8], range: Range<usize>) -> u8 {
    dbg!(bits);
    let mut byte = 0;
    for (i, b) in bits[range].iter().enumerate() {
        byte |= b << i
    }

    byte
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GenericCrc;

    // from usb crc paper: 
    // https://www.usb.org/sites/default/files/crcdes.pdf


    #[test]
    fn raw() {
        use crc::{Crc, CRC_5_USB};
        let crc = Crc::<u8>::new(&CRC_5_USB);
        let mut digest = crc.digest();
        // let mut digest = crc.digest_with_initial(0b11111111);

        // let bits = [1,0,1,0,1,0,0,0,1,1,1];
        let bits = [1,1,1,0,0,0,1,0,1,0,1]; // flipped from example
        // for byte in bits.chunks(8).map(|c| extract_byte(c, 0..c.len())) {
        //     dbg!(byte);
        //     digest.update(&[byte]);
        // }
        let byte0 = extract_byte(&bits, 0..7);
        let byte1 = extract_byte(&bits, 7..7+4);
        digest.update(&[byte0, byte1]);

        let crc = digest.finalize();
        assert_eq!(crc, 0b11101); // flipped from example
    }

    // #[test] // addr 15 endp e
    // fn crc_calc0() {
    //     // let bits = [1,0,1,0,1,0,0,0,1,1,1];
    //     let bits = [0,1,0,1,0,1,1,1,0,0,0];
    //     assert_eq!(bits.len(), 11);
    //
    //     assert_eq!(GenericCrc::calc(&bits), 0b10111);
    //     // assert_eq!(GenericCrc::calc(&bits), 0b01000);
    // }

    // #[test] 
    // fn crc_calc() {
    //     let bits = [1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0];
    //     assert_eq!(GenericCrc::calc(&bits), 0x04);
    // }

    // #[test]
    // fn crc_calc2() {
    //     let bits = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    //     assert_eq!(GenericCrc::calc(&bits), 0x02);
    // }
}
