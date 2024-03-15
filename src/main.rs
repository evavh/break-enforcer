#![feature(thread_sleep_until)]

use std::{
    fs::File,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, RecvTimeoutError},
        Arc,
    },
    thread,
};

mod check_inputs;
mod cli;
mod config;
mod lock;
mod notification;
mod setup;
mod watch;

use clap::Parser;

use crate::check_inputs::inactivity_watcher;
use crate::check_inputs::wait_for_any_input;
use crate::lock::Device;
use crate::notification::notify_all_users;

// For monitoring input
const MOUSE_DEVICE: &str = "/dev/input/mice";
const KEYBOARD_DEVICE: &str = "/dev/input/by-id/usb-046a_010d-event-kbd";
const ALL_DEVICES: [&str; 2] = [MOUSE_DEVICE, KEYBOARD_DEVICE];

// For blocking input
const MOUSE_NAMES: [&str; 2] = ["HSMshift", "Hippus N.V. HSMshift"];
const KEYBOARD_NAME: &str = "HID 046a:010d";

fn main() {
    let devices = watch::devices().unwrap();

    let args = cli::Cli::parse();
    let (work_duration, break_duration, grace_duration) = match args.command {
        cli::Commands::Run {
            work_duration,
            break_duration,
            grace_duration,
        } => (work_duration, break_duration, grace_duration),
        cli::Commands::Wizard => {
            setup::wizard(&devices, args.config_path).unwrap();
            return;
        }
    };

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
                work_duration,
            );
        });
    }

    loop {
        notify_all_users("Waiting for input to start work timer...");
        block_on_new_input(&recv_any_input);
        notify_all_users(&format!("Starting work timer for {break_duration:?}"));
        work_start_sender.send(true).unwrap();
        match break_skip_receiver.recv_timeout(break_duration - grace_duration) {
            Ok(_) => {
                notify_all_users("No input for breaktime");
                block_on_new_input(&recv_any_input);
                break_skip_is_sent.store(false, Ordering::Release);
                continue;
            }
            Err(RecvTimeoutError::Timeout) => (),
            Err(e) => panic!("Unexpected error: {e}"),
        }

        notify_all_users(&format!("Locking in {grace_duration:?}!"));
        thread::sleep(grace_duration);

        let mut locks = Vec::new();

        for mouse in MOUSE_NAMES.map(find_event).into_iter().flatten() {
            locks.push(mouse.clone().lock().unwrap());
        }
        for keyboard in find_event(KEYBOARD_NAME) {
            locks.push(keyboard.clone().lock().unwrap());
        }

        notify_all_users(&format!("Starting break timer for {work_duration:?}"));
        thread::sleep(work_duration);

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

    devices.into_iter().filter(|x| x.name == name).collect()
}
