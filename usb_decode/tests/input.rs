#![allow(unused_attributes)]
#![feature(once_cell)]

use std::sync::LazyLock;

pub static DATA: LazyLock<Vec<Vec<u8>>> = LazyLock::new(|| read_data());

fn read_data() -> Vec<Vec<u8>> {
    let text =
        std::fs::read_to_string("tests/input.txt").expect("Failed to open stored spy output");
    let (_, text) = text.split_once('[').unwrap();
    let (text, _) = text.rsplit_once(']').unwrap();
    let arrays = text.split(",");

    arrays
        .map(str::trim)
        .map(|s| s.trim_matches(|c| c == '[' || c == ']'))
        .map(str::trim)
        .map(|array| {
            array
                .chars()
                .map(|c| {
                    c.to_digit(2)
                        .expect(&format!("Parse error on input: '{c}'"))
                })
                .map(|d| d as u8)
                .collect()
        })
        .collect()
}

#[test]
fn data_not_empty() {
    assert!(!DATA.is_empty());
    for sample in DATA.iter() {
        assert!(sample.len() > 10);
    }
}

#[test]
fn sync_present_in_data() {
    for sample in DATA.iter() {
        assert_eq!(sample[0], 1);
        assert_eq!(sample[1], 0);
        assert_eq!(sample[2], 1);
    }
}
