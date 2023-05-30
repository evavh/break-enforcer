// #![no_std]

mod header;
mod adress;
mod data;
mod crc;

pub type Error = &'static str;

use adress::Address;
use header::Header;
use data::Data;
pub use crate::crc::{GenericCrc, Crc5};

fn eat_partial_preamble(data: &[u8]) -> Result<&[u8], Error> {
    const END_OF_PREAMBLE: [u8; 5] = [1, 0, 1, 0, 0];
    if data[0..5] == END_OF_PREAMBLE {
        Ok(&data[5..])
    } else {
        Err("Wrong preamble")
    }
}


pub enum Packet {
    // only send by the host
    In(Address),
    // only send by the host
    Out(Address),
    DataEven(Data),
    DataOdd(Data),
    Unknown,
}

pub fn decode<C: Crc5>(bits: &[u8]) -> Result<(&[u8], Packet), Error> {
    let bits = eat_partial_preamble(bits)?;
    let (bits, header) = header::parse(bits)?;

    Ok(match header {
        Header::IN => {
            let (bits, addr) = adress::parse::<C>(bits)?;
            (bits, Packet::In(addr))
        }
        Header::OUT => {
            let (bits, addr) = adress::parse::<C>(bits)?;
            (bits, Packet::Out(addr))
        }
        Header::DATA0 => {
            let (bits, data) = data::parse(bits)?;
            (bits, Packet::DataEven(data))
        }
        Header::DATA1 => {
            let (bits, data) = data::parse(bits)?;
            (bits, Packet::DataOdd(data))
        }
        _ => {
            // find EOF, update `bits`
            (bits, Packet::Unknown)
        }
    })
}

#[cfg(test)]
mod tests {
    use encode_derive::nrzi_low;

    #[test]
    fn preamble() {
        let line = nrzi_low!(0, 01011010);
        assert_eq!(line, [1, 1, 0, 0, 0, 1, 1, 0])
    }
}
