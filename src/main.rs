use std::{
    sync::mpsc::{channel, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

use crate::check_inputs::wait_for_input;

use self::check_inputs::send_when_breaktime_inactive;

const MOUSE_DEVICE: &'static str = "/dev/input/mice";
const KEYBOARD_DEVICE: &'static str =
    "/dev/input/by-id/usb-046a_010d-event-kbd";

const T_BREAK: Duration = Duration::from_secs(5);
const T_WORK: Duration = Duration::from_secs(15);

mod check_inputs;

fn main() {
    let (send, recv) = channel();

    thread::spawn(move || send_when_breaktime_inactive(KEYBOARD_DEVICE, send));

    loop {
        println!("Keyboard on!");
        match recv.recv_timeout(T_WORK) {
            Ok(_) => {
                println!("No input for breaktime");
                wait_for_input(KEYBOARD_DEVICE);
                println!("Restarting work");
                continue;
            }
            Err(RecvTimeoutError::Timeout) => (),
            Err(e) => panic!("Unexpected error: {e}"),
        }
        println!("Keyboard off!");
        thread::sleep(T_BREAK);
    }
}
