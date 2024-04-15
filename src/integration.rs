use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use color_eyre::Result;

use crate::install::fmt_dur;

mod file_status;
use file_status::FileStatus;
mod notification;

#[derive(Debug, PartialEq, Eq)]
enum State {
    Waiting,
    Work { next_break: Instant },
    Break { next_work: Instant },
}

trait DurationUntil {
    fn duration_until(&self) -> Duration;
}

impl DurationUntil for Instant {
    fn duration_until(&self) -> Duration {
        self.saturating_duration_since(Instant::now())
    }
}

pub struct Status {
    update: mpsc::Sender<State>,
    integrator: Option<JoinHandle<Result<()>>>,
    lock_warning_notification: Option<Duration>,
}

fn integrate(
    rx: &mpsc::Receiver<State>,
    mut file_status: Option<FileStatus>,
    state_notifications: bool,
    idle: Arc<Mutex<Instant>>,
    break_duration: Duration,
) -> Result<()> {
    let mut timeout = Duration::MAX;
    let mut state = State::Waiting;

    loop {
        let mut state_changed = false;
        match rx.recv_timeout(timeout) {
            Ok(s) => {
                state = s;
                state_changed = true;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }

        timeout = match state {
            State::Waiting => Duration::MAX,
            State::Work { .. } | State::Break { .. } => Duration::from_secs(1),
        };

        let msg = match state {
            State::Waiting => String::from("-"),
            State::Work { next_break } => {
                let idle = idle.lock().unwrap().elapsed();
                let break_dur = break_duration.saturating_sub(idle);
                let break_dur = fmt_dur(break_dur);
                let next_break = fmt_dur(next_break.duration_until());
                format!("{} break in: {}", break_dur, next_break)
            }
            State::Break { next_work } => {
                format!("unlocks in: {}", fmt_dur(next_work.duration_until()))
            }
        };

        if let Some(file_status) = &mut file_status {
            file_status.update(&msg);
        }
        if state_notifications && state_changed {
            notification::notify_all_users(&msg);
        }
    }
}

impl Status {
    pub fn new(
        file_integration: bool,
        notifications: bool,
        lock_warning_notification: Option<Duration>,
        idle: Arc<Mutex<Instant>>,
        break_duration: Duration,
    ) -> Result<Self> {
        let file_status = if file_integration {
            Some(FileStatus::new()?)
        } else {
            None
        };

        let (tx, rx) = mpsc::channel();
        let integrator =
            thread::spawn(move || integrate(&rx, file_status, notifications, idle, break_duration));
        Ok(Self {
            update: tx,
            integrator: Some(integrator),
            lock_warning_notification,
        })
    }

    fn send(&mut self, new_state: State) {
        let res = self.update.send(new_state);
        if res.is_err() {
            // Get issues from the integrator thread and crash here on the main
            // thread. That way the program will exit.
            self.integrator
                .take()
                .expect("can only be called once")
                .join()
                .expect("The integrator thread panicked")
                .expect("The integrator thread returned an error, it should not");
        }
    }

    pub(crate) fn set_waiting(&mut self) {
        self.send(State::Waiting);
    }

    pub(crate) fn set_working(&mut self, next_break: Instant) {
        self.send(State::Work { next_break });

        if let Some(before_break) = self.lock_warning_notification {
            thread::spawn(move || {
                let msg = format!("locking in {}", fmt_dur(before_break));
                #[allow(clippy::pedantic)]
                thread::sleep_until(next_break - before_break);
                notification::notify_all_users(&msg);
            });
        }
    }

    pub(crate) fn set_break(&mut self, next_work: Instant) {
        self.send(State::Break { next_work });
    }
}
