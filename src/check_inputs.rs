use std::{
    fs::{self, File},
    io::{self, Read},
    sync::{
        mpsc::{self, channel, Receiver, RecvTimeoutError, Sender, TryRecvError},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use color_eyre::eyre::Context;

use crate::{config::InputFilter, watch_and_block::NewInput};

pub struct InactivityTracker {
    idle_since: Arc<Mutex<Instant>>,
    reset_notify: mpsc::Receiver<color_eyre::Result<()>>,
}

pub enum TrackResult {
    ShouldReset,
    ShouldBreak { user_idle: Duration },
    Error(color_eyre::Report),
}

impl InactivityTracker {
    pub fn new(input_receiver: Receiver<InputResult>, break_duration: Duration) -> Self {
        let idle_since = Arc::new(Mutex::new(Instant::now()));
        let (tx, rx) = mpsc::channel();
        {
            let idle_since = idle_since.clone();
            thread::spawn(move || watch_activity(&input_receiver, break_duration, idle_since, tx));
        }

        Self {
            idle_since,
            reset_notify: rx,
        }
    }
    pub fn reset_or_timeout(&mut self, work_duration: Duration) -> TrackResult {
        // Empty the reset_notify. At this point in the program we just left a
        // period without input (waiting or break). Therefore there has been no user
        // activity until here. Any reset notification received after emptying
        // the channel must have been send after the period without input and
        // therefore at least a break duration must have elapsed.
        loop {
            match self.reset_notify.try_recv() {
                Ok(Err(e)) => return TrackResult::Error(e),
                Ok(Ok(())) => (),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => unreachable!(),
            }
        }

        match self.reset_notify.recv_timeout(work_duration) {
            Ok(Ok(())) => TrackResult::ShouldReset,
            Ok(Err(e)) => TrackResult::Error(e),
            Err(RecvTimeoutError::Timeout) => TrackResult::ShouldBreak {
                user_idle: self.idle_since.lock().unwrap().elapsed(),
            },
            Err(RecvTimeoutError::Disconnected) => unreachable!(),
        }
    }

    pub fn idle_handle(&self) -> Arc<Mutex<Instant>> {
        self.idle_since.clone()
    }
}

fn watch_activity(
    input_receiver: &Receiver<InputResult>,
    break_duration: Duration,
    idle_since: Arc<Mutex<Instant>>,
    reset_notify: mpsc::Sender<color_eyre::Result<()>>,
) {
    loop {
        match input_receiver.recv_timeout(break_duration) {
            Ok(Ok(())) => *idle_since.lock().unwrap() = Instant::now(),
            Err(RecvTimeoutError::Timeout) => reset_notify.send(Ok(())).unwrap(),
            Err(RecvTimeoutError::Disconnected) => unreachable!(),
            Ok(err @ Err(_)) => {
                let err = err.wrap_err("test");
                reset_notify.send(err).unwrap();
            }
        }
    }
}

pub type InputResult = Result<(), Arc<io::Error>>;

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
    tx1: &Sender<InputResult>,
    tx2: &Sender<InputResult>,
) {
    let mut file = match fs::File::open(input.path) {
        // means the device is disconnected
        Err(e) if e.kind() == io::ErrorKind::NotFound => return,
        Err(e) => {
            // unexpected error, report to main thread
            dbg!(&e);
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
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // device was disconnected
                break;
            }
            Err(e) if device_removed(&e) => {
                // device was disconnected
                break;
            }
            Err(e) => {
                // unexpected error, report to main thread
                let err = Arc::new(e); // make cloneable
                let _ig_err = tx1.send(Err(err.clone()));
                let _ig_err = tx2.send(Err(err));
                return;
            }
            Ok(()) => (),
        };

        let _ = tx1.send(Ok(()));
        let _ = tx2.send(Ok(()));
    }
}

pub fn wait_for_input(file: &mut File) -> std::io::Result<()> {
    let mut packet = [0u8; 24];
    file.read_exact(&mut packet)
}

pub fn device_removed(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(19i32) && e.to_string().contains("No such device")
}
