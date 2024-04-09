use std::{
    fs::{self, File},
    io::{self, Read},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
        Arc,
    },
    thread,
};

use crate::{config::InputFilter, watch::NewInput};

pub fn wait_for_input(file: &mut File) -> std::io::Result<()> {
    let mut packet = [0u8; 24];
    file.read_exact(&mut packet)
}

pub fn inactivity_watcher(
    work_start_receiver: &Receiver<bool>,
    break_skip_sender: &Sender<bool>,
    break_skip_sent: &Arc<AtomicBool>,
    input_receiver: &Receiver<InputResult>,
    break_duration: std::time::Duration,
) {
    work_start_receiver.recv().unwrap();

    loop {
        match input_receiver.recv_timeout(break_duration) {
            Ok(Ok(_)) => (),
            Ok(Err(_)) => return,
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

pub type InputResult = Result<bool, Arc<io::Error>>;

pub(crate) fn watcher(
    just_connected: Receiver<NewInput>,
    to_block: Vec<InputFilter>,
) -> (Receiver<InputResult>, Receiver<InputResult>) {
    let (tx1, rx1) = channel();
    let (tx2, rx2) = channel();

    thread::spawn(move || loop {
        let new_device = just_connected
            .recv()
            .expect("only disconnects at program exit");
        if !to_block
            .iter()
            .filter(|filter| filter.id == new_device.id)
            .any(|filter| filter.names.contains(&new_device.name))
        {
            continue;
        }

        let tx1 = tx1.clone();
        let tx2 = tx2.clone();
        thread::Builder::new()
            .spawn(move || monitor_input(new_device, &tx1, &tx2))
            .expect("the OS should be able to spawn a thread");
    });

    (rx1, rx2)
}

fn monitor_input(
    input: NewInput,
    tx1: &Sender<Result<bool, Arc<io::Error>>>,
    tx2: &Sender<Result<bool, Arc<io::Error>>>,
) {
    let mut file = match fs::File::open(input.path) {
        // means the device is disconnected
        Err(e) if e.kind() == io::ErrorKind::NotFound => return,
        Err(e) => {
            // unexpected error, report to main thread
            let err = Arc::new(e); // make cloneable
            let _ig_err = tx1.send(Err(err.clone()));
            let _ig_err = tx2.send(Err(err));
            return;
        }
        Ok(file) => file,
    };
    loop {
        match wait_for_input(&mut file) {
            // means the device is disconnected
            Err(e) if e.kind() == io::ErrorKind::NotFound => return,
            Err(e) => {
                // unexpected error, report to main thread
                let err = Arc::new(e); // make cloneable
                let _ig_err = tx1.send(Err(err.clone()));
                let _ig_err = tx2.send(Err(err));
                return;
            }
            Ok(()) => (),
        };

        let _ = tx1.send(Ok(true));
        let _ = tx2.send(Ok(true));
    }
}
