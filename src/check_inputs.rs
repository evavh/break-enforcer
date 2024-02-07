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

// TODO get file open out of here, runs too often
pub fn wait_for_input(file: &mut File) -> Instant {
    let mut packet = [0u8; 24];
    file.read_exact(&mut packet).unwrap();

    Instant::now()
}

pub fn wait_for_any_input(files: &mut [File; 2]) -> Instant {
    let (send, recv) = channel();

    for file in files {
        let send = send.clone();

        thread::Builder::new()
            .spawn(move || {
                wait_for_input(&mut file);
                let _ = send.send(true);
            })
            .unwrap();
    }

    recv.recv().unwrap();
    Instant::now()
}

pub fn inactivity_watcher(
    devices: [&'static str; 2],
    work_start_receiver: &Receiver<bool>,
    break_skip_sender: &Sender<bool>,
    break_skip_sent: &Arc<AtomicBool>,
) {
    let mut files = devices.map(File::open).map(Result::unwrap);

    work_start_receiver.recv().unwrap();

    let (input_sender, input_receiver) = channel();
    thread::spawn(move || loop {
        wait_for_any_input(&mut files);
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
