use std::{
    fs::File,
    io::Read,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
        Arc,
    },
    thread,
};

use crate::T_BREAK;

pub fn wait_for_input(file: &mut File) {
    let mut packet = [0u8; 24];
    file.read_exact(&mut packet).unwrap();
}

pub fn wait_for_any_input(files: [File; 2]) -> Receiver<bool> {
    let (send, recv) = channel();

    for mut file in files {
        let send = send.clone();

        thread::Builder::new()
            .spawn(move || loop {
                wait_for_input(&mut file);
                let _ = send.send(true);
            })
            .unwrap();
    }

    recv
}

pub fn inactivity_watcher(
    work_start_receiver: &Receiver<bool>,
    break_skip_sender: &Sender<bool>,
    break_skip_sent: &Arc<AtomicBool>,
    input_receiver: &Receiver<bool>,
) {
    work_start_receiver.recv().unwrap();

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
