use std::{
    fs::File,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, RecvTimeoutError},
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

// For monitoring input
const MOUSE_DEVICE: &str = "/dev/input/mice";
const KEYBOARD_DEVICE: &str = "/dev/input/by-id/usb-046a_010d-event-kbd";
const ALL_DEVICES: [&str; 2] = [MOUSE_DEVICE, KEYBOARD_DEVICE];

// For blocking input
const MOUSE_NAMES: [&str; 2] = ["HSMshift", "Hippus N.V. HSMshift"];
const KEYBOARD_NAME: &str = "HID 046a:010d";

const T_BREAK: Duration = Duration::from_secs(5 * 60);
const T_WORK: Duration = Duration::from_secs(15 * 60);

fn main() {
    let device_files = ALL_DEVICES.map(File::open).map(Result::unwrap);
    let device_files2 = ALL_DEVICES.map(File::open).map(Result::unwrap);

    let (break_skip_sender, break_skip_receiver) = channel();
    let (work_start_sender, work_start_receiver) = channel();
    let break_skip_is_sent = Arc::new(AtomicBool::new(false));

    let recv_any_input = wait_for_any_input(device_files);
    let recv_any_input2 = wait_for_any_input(device_files2);

    {
        let break_skip_is_sent = break_skip_is_sent.clone();

        thread::spawn(move || {
            inactivity_watcher(
                &work_start_receiver,
                &break_skip_sender,
                &break_skip_is_sent,
                &recv_any_input2,
            );
        });
    }

    loop {
        notify_all_users("Waiting for input to start work timer...");
        block_on_new_input(&recv_any_input);
        notify_all_users(&format!("Starting work timer for {T_WORK:?}"));
        work_start_sender.send(true).unwrap();
        match break_skip_receiver.recv_timeout(T_WORK) {
            Ok(_) => {
                notify_all_users("No input for breaktime");
                block_on_new_input(&recv_any_input);
                break_skip_is_sent.store(false, Ordering::Release);
                continue;
            }
            Err(RecvTimeoutError::Timeout) => (),
            Err(e) => panic!("Unexpected error: {e}"),
        }

        let mut locks = Vec::new();

        for mouse in MOUSE_NAMES.map(find_event).into_iter().flatten() {
            locks.push(mouse.clone().lock().unwrap());
        }
        for keyboard in find_event(KEYBOARD_NAME) {
            locks.push(keyboard.clone().lock().unwrap());
        }

        notify_all_users(&format!("Starting break timer for {T_BREAK:?}"));
        thread::sleep(T_BREAK);

        for lock in locks {
            lock.unlock();
        }
    }
}

fn block_on_new_input(recv_any_input: &Receiver<bool>) {
    loop {
        if recv_any_input.try_recv().is_err() {
            break;
        };
    }

    recv_any_input.recv().unwrap();
}

fn find_event(name: &str) -> Vec<Device> {
    let devices = lock::list_devices();

    dbg!(devices.into_iter().filter(|x| x.name == name).collect())
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
        .filter(|x| x.starts_with(' '))
        .map(|x| x.split(' ').filter(|x| !x.is_empty()))
        .map(|mut x| (x.nth(1).unwrap(), x.next().unwrap()));

    for (uid, username) in users {
        notify(username, uid, text);
    }
}
