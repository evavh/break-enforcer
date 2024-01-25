use std::{fs::File, io::Read, time::Instant};

pub fn wait_for_input(device: &str) -> Instant {
    let mut file = File::open(device).unwrap();
    let mut packet = [0u8; 24];
    file.read_exact(&mut packet).unwrap();

    Instant::now()
}
