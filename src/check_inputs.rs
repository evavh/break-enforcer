use std::{
    fs::File,
    io::Read,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
        Arc,
    },
    thread,
    time::Instant,
};

use crate::T_BREAK;

pub fn wait_for_input(device: &str) -> Instant {
    let mut file = File::open(device).unwrap();
    let mut packet = [0u8; 24];
    file.read_exact(&mut packet).unwrap();

    Instant::now()
}

pub fn inactivity_watcher(
    device: &'static str,
    work_start_receiver: &Receiver<bool>,
    break_skip_sender: &Sender<bool>,
    break_skip_sent: &Arc<AtomicBool>,
) {
    work_start_receiver.recv().unwrap();

    let (input_sender, input_receiver) = channel();
    thread::spawn(move || loop {
        wait_for_input(device);
        input_sender.send(true).unwrap();
    });

    loop {
        match input_receiver.recv_timeout(T_BREAK) {
            Ok(_) => (),
            Err(RecvTimeoutError::Timeout) => {
                if !break_skip_sent.load(Ordering::Acquire) {
                    break_skip_sender.send(true).unwrap();
                    break_skip_sent.store(true, Ordering::Release);
                }
            }
            Err(e) => panic!("Unexpected error: {e}"),
        }
    }
}
