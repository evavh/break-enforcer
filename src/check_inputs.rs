use std::{
    fs::File,
    io::Read,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, RecvTimeoutError, Sender}, Arc,
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

pub fn send_when_breaktime_inactive(
    device: &'static str,
    outer_send: Sender<bool>,
    break_skip_sent: Arc<AtomicBool>,
) {
    let (inner_send, inner_recv) = channel();
    thread::spawn(move || loop {
        wait_for_input(device);
        inner_send.send(true).unwrap();
    });

    loop {
        match inner_recv.recv_timeout(T_BREAK) {
            Ok(_) => (),
            Err(RecvTimeoutError::Timeout) => {
                if !break_skip_sent.load(Ordering::Acquire) {
                    outer_send.send(true).unwrap();
                    break_skip_sent.store(true, Ordering::Release);
                }
            }
            Err(e) => panic!("Unexpected error: {e}"),
        }
    }
}
