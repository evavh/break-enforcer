// this is shared between a lib and a bin target (main.rs). We do not want to
// share the internal details from the bin target to the lib. Thats why this is
// a separate module and not part of the integrations mod

pub(crate) const STOP_BYTE: u8 = 0;
// first 4 are taken with care from
// https://en.wikipedia.org/wiki/List_of_TCP_and_UDP_port_numbers
// the rest are randomly picked
pub(crate) const PORTS: [u16; 7] = [49_151, 28_769, 19_788, 62_738, 34_342, 12_846, 8_797];
