use std::{
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, RecvTimeoutError},
        Arc,
    },
    thread,
    time::Duration,
};

mod check_inputs;

use crate::check_inputs::inactivity_watcher;
use crate::check_inputs::wait_for_any_input;

const USER: &str = "focus";
const UID: &str = "1004";

const MOUSE_DEVICE: &str = "/dev/input/mice";
const KEYBOARD_DEVICE: &str = "/dev/input/by-id/usb-046a_010d-event-kbd";
const ALL_DEVICES: [&str; 2] = [MOUSE_DEVICE, KEYBOARD_DEVICE];

const T_BREAK: Duration = Duration::from_secs(5);
const T_WORK: Duration = Duration::from_secs(15);

fn main() {
    let (break_skip_sender, break_skip_receiver) = channel();
    let (work_start_sender, work_start_receiver) = channel();
    let break_skip_is_sent = Arc::new(AtomicBool::new(false));

    {
        let break_skip_is_sent = break_skip_is_sent.clone();

        thread::spawn(move || {
            inactivity_watcher(
                ALL_DEVICES,
                &work_start_receiver,
                &break_skip_sender,
                &break_skip_is_sent,
            );
        });
    }

    loop {
        notify("Keyboard on!");
        notify("Waiting for input to start work timer...");
        wait_for_any_input(ALL_DEVICES);
        notify("Starting work timer for {T_WORK:?}");
        work_start_sender.send(true).unwrap();
        match break_skip_receiver.recv_timeout(T_WORK) {
            Ok(_) => {
                notify("No input for breaktime, skip break");
                notify("Waiting for input to restart work timer...");
                wait_for_any_input(ALL_DEVICES);
                notify("Restarting work");
                break_skip_is_sent.store(false, Ordering::Release);
                continue;
            }
            Err(RecvTimeoutError::Timeout) => (),
            Err(e) => panic!("Unexpected error: {e}"),
        }
        notify("Keyboard off!");
        notify("Starting break timer for {T_BREAK:?}");
        thread::sleep(T_BREAK);
    }
}

fn notify(text: &str) {
    let command = format!("sudo -u {USER} DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{UID}/bus notify-send -t 5000 \"{text}\"");
    Command::new("sh").arg("-c").arg(command).output().unwrap();
}
