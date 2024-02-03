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
mod lock;

use crate::check_inputs::inactivity_watcher;
use crate::check_inputs::wait_for_any_input;
use crate::lock::Device;

const MOUSE_DEVICE: &str = "/dev/input/mice";
const MOUSE_EVENT: &str = "/dev/input/event14";
const KEYBOARD_DEVICE: &str = "/dev/input/by-id/usb-046a_010d-event-kbd";
const KEYBOARD_EVENT: &str = "/dev/input/event3";
const ALL_DEVICES: [&str; 2] = [MOUSE_DEVICE, KEYBOARD_DEVICE];

const T_BREAK: Duration = Duration::from_secs(5 * 60);
const T_WORK: Duration = Duration::from_secs(15 * 60);

fn main() {
    let keyboard_dev = Device {
        event_path: KEYBOARD_EVENT.to_string(),
        name: "HID 046a:010d".to_string(),
    };
    let mouse_dev = Device {
        event_path: MOUSE_EVENT.to_string(),
        name: "HSMshift".to_string(),
    };

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
        notify_all_users("Keyboard on!");
        notify_all_users("Waiting for input to start work timer...");
        wait_for_any_input(ALL_DEVICES);
        notify_all_users(&format!("Starting work timer for {T_WORK:?}"));
        work_start_sender.send(true).unwrap();
        match break_skip_receiver.recv_timeout(T_WORK) {
            Ok(_) => {
                notify_all_users("No input for breaktime, skip break");
                notify_all_users("Waiting for input to restart work timer...");
                wait_for_any_input(ALL_DEVICES);
                notify_all_users("Restarting work");
                break_skip_is_sent.store(false, Ordering::Release);
                continue;
            }
            Err(RecvTimeoutError::Timeout) => (),
            Err(e) => panic!("Unexpected error: {e}"),
        }
        {
            let _mouse = mouse_dev.clone().lock();
            let _keyboard = keyboard_dev.clone().lock();

            notify_all_users("Keyboard off!");
            notify_all_users(&format!("Starting break timer for {T_BREAK:?}"));
            thread::sleep(T_BREAK);
        }
    }
}

fn notify(username: &str, uid: &str, text: &str) {
    let command = format!("sudo -u {username} DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{uid}/bus notify-send -t 5000 \"{text}\"");
    Command::new("sh").arg("-c").arg(command).output().unwrap();
}

fn notify_all_users(text: &str) {
    let users = Command::new("loginctl").output().unwrap().stdout;
    let users = String::from_utf8(users).unwrap();
    let users = users
        .lines()
        .filter(|x| x.starts_with(" "))
        .map(|x| x.split(' '))
        .map(|mut x| (x.nth(5).unwrap(), x.next().unwrap()));

    for (uid, username) in users {
        notify(username, uid, text);
    }
}
