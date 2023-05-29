#![no_std]

pub type Error = &'static str;
use encode_derive::nrzi_high;

fn eat_preamble(data: &[u8]) -> Result<&[u8], Error> {
    const END_OF_PREAMBLE: [u8; 5] = [1, 0, 1, 0, 0];
    if data[0..5] == END_OF_PREAMBLE {
        Ok(&data[5..])
    } else {
        Err("Wrong preamble")
    }
}

pub enum Header {
    /// High-bandwidth (USB 2.0) split transaction
    SPLIT,
    /// Check if endpoint can accept data (USB 2.0)
    PING,
    /// Low-bandwidth USB preamble
    PRE,
    /// Split transaction error (USB 2.0)
    ERR,
    /// Data packet accepted
    ACK,
    /// Data packet not accepted; please retransmit
    NAK,
    /// Data not ready yet (USB 2.0)
    NYET,
    /// Transfer impossible; do error recovery
    STALL,
    /// Address for host-to-device transfer
    OUT,
    /// Address for device-to-host transfer
    IN,
    /// Start of frame marker (sent each ms)
    SOF,
    /// Address for host-to-device control transfer
    SETUP,
    /// Even-numbered data packet
    DATA0,
    /// Odd-numbered data packet
    DATA1,
    /// Data packet for high-bandwidth isochronous transfer (USB 2.0)
    DATA2,
    /// Data packet for high-bandwidth isochronous transfer (USB 2.0)
    MDATA,
}

fn parse_header(data: &[u8]) -> Result<(&[u8], Header), Error> {
    let header = match data[..8] {
        nrzi_high!(1, 0001 1110) => Header::SPLIT,
        nrzi_high!(1, 0010 1101) => Header::PING,
        nrzi_high!(1, 0011 1100) => Header::PRE,
        nrzi_high!(1, 0011 1100) => Header::ERR,
        nrzi_high!(1, 0100 1011) => Header::ACK,
        nrzi_high!(1, 0101 1010) => Header::NAK,
        nrzi_high!(1, 0110 1001) => Header::NYET,
        nrzi_high!(1, 0111 1000) => Header::STALL,
        nrzi_high!(1, 1000 0111) => Header::OUT,
        nrzi_high!(1, 1001 0110) => Header::IN,
        nrzi_high!(1, 1010 0101) => Header::SOF,
        nrzi_high!(1, 1011 0100) => Header::SETUP,
        nrzi_high!(1, 1100 0011) => Header::DATA0,
        nrzi_high!(1, 1101 0010) => Header::DATA1,
        nrzi_high!(1, 1110 0001) => Header::DATA2,
        nrzi_high!(1, 1111 0000) => Header::MDATA,
        _ => return Err("Corrupt header"),
    };
    Ok((&data[8..], header))
}

pub fn decode(data: &[u8]) -> Result<(), Error> {
    let data = eat_preamble(data)?;
    let (data, header) = parse_header(data)?;

    Ok(())
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
