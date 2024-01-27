use std::{sync::mpsc::channel, thread, time::{Instant, Duration}};

use crate::check_inputs::wait_for_input;

const MOUSE_DEVICE: &'static str = "/dev/input/mice";
const KEYBOARD_DEVICE: &'static str =
    "/dev/input/by-id/usb-046a_010d-event-kbd";

mod check_inputs;

fn main() {
    let mut n_inputs = 0;
    let mut input_times: Vec<Instant> = Vec::new();
    let program_start = Instant::now();

    let (send, recv) = channel();

    wait_for_input(KEYBOARD_DEVICE);

    let send1 = send.clone();
    thread::spawn(move || loop {
        let time = wait_for_input(KEYBOARD_DEVICE);
        send.send(time).unwrap();
    });
    thread::spawn(move || loop {
        let time = wait_for_input(MOUSE_DEVICE);
        send1.send(time).unwrap();
    });

    loop {
        let input_time = recv.recv_timeout(Duration::from_secs(5)).unwrap();
        println!("Got input time {input_time:?}");
    }
}
