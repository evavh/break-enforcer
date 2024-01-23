use crate::check_inputs::wait_for_input;

const MOUSE_DEVICE: &'static str = "/dev/input/mice";
const KEYBOARD_DEVICE: &'static str =
    "/dev/input/by-id/usb-046a_010d-event-kbd";

mod check_inputs;

fn main() {
    let mut n_inputs = 0;
    loop {
        wait_for_input(KEYBOARD_DEVICE);
        n_inputs += 1;
        println!("{n_inputs} inputs detected\r");
    }
}
