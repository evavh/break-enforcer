#![crate_type = "proc-macro"]

mod diff;
mod nrzi;

extern crate proc_macro;
use diff::Side;
use proc_macro::TokenStream;

/// encode to nrzi then to differential signalling
/// output the signal for the normally high line
///
/// arguments (comma seperated):
///   - bit of the data send before this
///   - bitstring of the data to send
/// 'returns' an array
///
/// example usage:
/// let PID = nrzi_high!(0, 1001);
/// const PID: [u8, 4] = nrzi_high!(0, 1001);
#[proc_macro]
pub fn nrzi_high(item: TokenStream) -> TokenStream {
    let input = item.to_string();
    let (prev_bit, bits) = parse_input(input);
    let nrzi = nrzi::encode(prev_bit, &bits);
    let signal = diff::encode(Side::NormallyHigh, &nrzi);

    format!("{signal:?}").parse().unwrap()
}

/// encode to nrzi then to differential signalling
/// output the signal for the normally low line
///
/// arguments (comma seperated):
///   - bit of the data send before this
///   - bitstring of the data to send
/// 'returns' an array
///
/// example usage:
/// let PID = nrzi_high!(0, 1001);
/// const PID: [u8, 4] = nrzi_high!(0, 1001);
#[proc_macro]
pub fn nrzi_low(item: TokenStream) -> TokenStream {
    let input = item.to_string();
    let (prev_bit, bits) = parse_input(input);
    let nrzi = nrzi::encode(prev_bit, &bits);
    let signal = diff::encode(Side::NormallyLow, &nrzi);

    format!("{signal:?}").parse().unwrap()
}

fn parse_input(item: String) -> (u8, Vec<u8>) {
    let (start, bitstring) = item.split_once(",").expect("syntax error: comma in input");
    let prev_bit = start.trim().parse().expect("previous bit must be a digit");
    let bits = bitstring
        .trim()
        .chars()
        .filter_map(|c| c.to_digit(2))
        .map(|d| d as u8)
        .collect();
    (prev_bit, bits)
}
