use crate::nrzi::Bit;

#[derive(Debug, Clone, Copy)]
pub enum Side {
    NormallyHigh = 1,
    NormallyLow = 0,
}

// assuming full speed
pub fn encode(side: Side, data: &[Bit]) -> Vec<Bit> {
    data.iter().map(|bit| {
        match (side, bit) {
            (Side::NormallyHigh, 0) => 1,
            (Side::NormallyHigh, 1) => 0,
            (Side::NormallyLow, 0) => 0,
            (Side::NormallyLow, 1) => 1,
            _ => unreachable!(),
        }
    }).collect()
}
