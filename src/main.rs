use std::time::Instant;

use crate::check_inputs::wait_for_input;

const MOUSE_DEVICE: &'static str = "/dev/input/mice";
const KEYBOARD_DEVICE: &'static str =
    "/dev/input/by-id/usb-046a_010d-event-kbd";

mod check_inputs;

fn main() {
    let mut n_inputs = 0;
    let mut input_times: Vec<Instant> = Vec::new();
    let program_start = Instant::now();

    loop {
        let input_time = wait_for_input(KEYBOARD_DEVICE);
        input_times.push(input_time.duration_since(program_start).as_secs());
        n_inputs += 1;
        println!("{n_inputs} inputs detected\r");
    }
}
