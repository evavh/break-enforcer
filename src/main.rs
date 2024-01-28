use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, RecvTimeoutError},
        Arc,
    },
    thread,
    time::Duration,
};

use crate::check_inputs::wait_for_input;

use self::check_inputs::inactivity_watcher;

const MOUSE_DEVICE: &str = "/dev/input/mice";
const KEYBOARD_DEVICE: &str = "/dev/input/by-id/usb-046a_010d-event-kbd";

const T_BREAK: Duration = Duration::from_secs(5);
const T_WORK: Duration = Duration::from_secs(15);

mod check_inputs;

fn main() {
    let (break_skip_sender, break_skip_receiver) = channel();
    let (work_start_sender, work_start_receiver) = channel();
    let break_skip_is_sent = Arc::new(AtomicBool::new(false));

    {
        let break_skip_is_sent = break_skip_is_sent.clone();

        thread::spawn(move || {
            inactivity_watcher(
                KEYBOARD_DEVICE,
                &work_start_receiver,
                &break_skip_sender,
                &break_skip_is_sent,
            );
        });
    }

    loop {
        println!("Keyboard on!");
        println!("Waiting for input to start work timer...");
        wait_for_input(KEYBOARD_DEVICE);
        println!("Starting work timer for {T_WORK:?}");
        work_start_sender.send(true).unwrap();
        match break_skip_receiver.recv_timeout(T_WORK) {
            Ok(_) => {
                println!("No input for breaktime, skip break");
                println!("Waiting for input to restart work timer...");
                wait_for_input(KEYBOARD_DEVICE);
                println!("\nRestarting work");
                break_skip_is_sent.store(false, Ordering::Release);
                continue;
            }
            Err(RecvTimeoutError::Timeout) => (),
            Err(e) => panic!("Unexpected error: {e}"),
        }
        println!("\nKeyboard off!");
        println!("Starting break timer for {T_BREAK:?}\n");
        thread::sleep(T_BREAK);
    }
}
