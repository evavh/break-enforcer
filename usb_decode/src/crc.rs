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

    // #[test]
    // fn raw() {
    //     use crc::{Crc, CRC_5_USB};
    //     let crc = Crc::<u8>::new(&CRC_5_USB);
    //     let mut digest = crc.digest();
    //     // let mut digest = crc.digest_with_initial(0b11111111);
    //
    //     // let bits = [1,0,1,0,1,0,0,0,1,1,1];
    //     let bits = [1,1,1,0,0,0,1,0,1,0,1]; // flipped from example
    //     // for byte in bits.chunks(8).map(|c| extract_byte(c, 0..c.len())) {
    //     //     dbg!(byte);
    //     //     digest.update(&[byte]);
    //     // }
    //     let byte0 = extract_byte(&bits, 0..7);
    //     let byte1 = extract_byte(&bits, 7..7+4);
    //     digest.update(&[byte0, byte1]);
    //
    //     let crc = digest.finalize();
    //     assert_eq!(crc, 0b10111);
    //     // assert_eq!(crc, 0b10111_u8.reverse_bits()); // flipped from example
    // }

    // #[test]
    // fn raw2() { // addr 3a enp a
    //     use crc::{Crc, CRC_5_USB};
    //     let crc = Crc::<u8>::new(&CRC_5_USB);
    //     let mut digest = crc.digest();
    //
    //     let bits = [0,1,0,1,1,1,0,0,1,0,1]; // not flipped
    //
    //     // right aligned in input
    //     let mut byte1 = 0;
    //     byte1 |= bits[10] << 0;
    //     byte1 |= bits[9] << 1;
    //     byte1 |= bits[8] << 2;
    //     byte1 |= bits[7] << 3;
    //     byte1 |= bits[6] << 4;
    //     byte1 |= bits[5] << 5;
    //     byte1 |= bits[4] << 6;
    //
    //     let mut byte0 = 0;
    //     byte1 |= bits[3] << 0;
    //     byte0 |= bits[2] << 1;
    //     byte0 |= bits[1] << 2;
    //     byte0 |= bits[0] << 3;
    //
    //     println!("byte0-byte1: {byte0:b}-{byte1:b}");
    //     digest.update(&[byte0, byte1]);
    //
    //     let crc = digest.finalize();
    //     // assert_eq!(crc, 0b11100_u8);
    //     assert_eq!(crc, 0b11100_u8.reverse_bits()); // flipped
    // }

    // #[test]
    // fn raw3() { // addr 70 enp 4
    //     use crc::{Crc, CRC_5_USB};
    //     let crc = Crc::<u8>::new(&CRC_5_USB);
    //     let mut digest = crc.digest();
    //
    //     // let bits = [0,0,0,0,1,1,1,0,0,1,0];
    //     let bits = [0,1,0,0,1,1,1,0,0,0,0];
    //
    //     let mut byte1 = 0;
    //     byte1 |= bits[10] << 0;
    //     byte1 |= bits[9] << 1;
    //     byte1 |= bits[8] << 2;
    //     byte1 |= bits[7] << 3;
    //     byte1 |= bits[6] << 4;
    //     byte1 |= bits[5] << 5;
    //     byte1 |= bits[4] << 6;
    //     byte1 |= bits[3] << 7;
    //
    //     let mut byte0 = 0;
    //     byte0 |= bits[2] << 0;
    //     byte0 |= bits[1] << 1;
    //     byte0 |= bits[0] << 2;
    //
    //     println!("byte0-byte1: {byte0:b}-{byte1:b}");
    //     digest.update(&[byte0, byte1]);
    //
    //     let crc = digest.finalize();
    //     assert_eq!(crc, 0b01110_u8);
    //     // assert_eq!(crc, 0b01110_u8.reverse_bits()); // flipped
    // }

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

    #[test]
    fn crc_calc2() {
        // needs padding to reverse
        assert_eq!(0b00000010, 0b01000_000_u8.reverse_bits());
        assert_eq!(0x02, 0b01000_000_u8.reverse_bits());

        use crc::{Crc, CRC_5_USB};
        let crc = Crc::<u8>::new(&CRC_5_USB);
        let mut digest = crc.digest();

        // let bits = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        let byte1 = 0b0000_0000;
        let byte0 = 0b1111_1000; // need to pad left side with ones?

        digest.update(&[byte0, byte1]);

        let crc = digest.finalize();
        println!("{crc:b}");
        dbg!(crc.reverse_bits());
        assert_eq!(crc, 0x02);
    }

    // // TODO: try another libary or even C code <30-05-23, dvdsk noreply@davidsk.dev> 
    #[test]
    fn crc_measured() {
        use crc::Crc;
        // use crc::CRC_5_USB;
        const CRC_5_USB: crc::Algorithm<u8> = crc::Algorithm::<u8> {
            width: 5,
            poly: 0x05,
            init: 0x1f,
            refin: true,
            refout: true,
            xorout: 0x1f,
            check: 0x19,
            residue: 0b01100,
        };

        let bits = [1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0];

        let inputs: Vec<u16> = vec![
            // zero padded
            0b00000_000_1000_0011, // right aligned, reversed
            0b00000_110_0000_1000, // right aligned
            0b1100_0001_000_00000, // left aligned
            0b0001_0000_011_00000, // left aligned, reversed
            // one padded
            0b11111_000_1000_0011, // right aligned, reversed
            0b11111_110_0000_1000, // right aligned
            0b1100_0001_000_11111, // left aligned
            0b0001_0000_011_11111, // left aligned, reversed
        ];

        for word in inputs {
            let crc = Crc::<u8>::new(&CRC_5_USB);
            let mut digest = crc.digest();
            digest.update(&word.to_be_bytes());
            let crc_be = digest.finalize();

            let crc = Crc::<u8>::new(&CRC_5_USB);
            let mut digest = crc.digest();
            digest.update(&word.to_le_bytes());
            let crc_le = digest.finalize();
            dbg!(0x1C, crc_be, crc_le);

            if crc_be == 0x1C || crc_le == 0x1C {
                return;
            }
        }
        panic!("no input found that leads to correct check sum");
    }
}
