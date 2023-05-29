#![feature(once_cell)]

use usb_decode::decode;

mod input;
use input::DATA;

#[test]
fn decode_runs() {
    for sample in DATA.iter() {
        decode(&sample).unwrap();
    }
}
