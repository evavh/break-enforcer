use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use color_eyre::Result;

mod file_status;
use file_status::FileStatus;
pub(crate) mod dbus;
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
    dbus_maintain: Option<JoinHandle<Result<()>>>,
}

fn integrate(
    rx: &mpsc::Receiver<State>,
    mut file_status: Option<FileStatus>,
    state_notifications: bool,
    idle: Arc<Mutex<Instant>>,
    break_duration: Duration,
    lock_warning_notification: Option<Duration>,
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

        let msg = format_status(&state, &idle, break_duration);
        /* TODO: dbus api status update <dvdsk> */
        if let Some(file_status) = &mut file_status {
            file_status.update(&msg);
        }
        notify_if_needed(
            &state,
            /* TODO: merge these bools into a struct called "notifyable events" <dvdsk> */
            lock_warning_notification,
            state_notifications,
            state_changed,
            msg,
        );
    }
}

fn notify_if_needed(
    state: &State,
    lock_warning_notification: Option<Duration>,
    state_notifications: bool,
    state_changed: bool,
    msg: String,
) {
    match *state {
        State::Work { next_break } => {
            if let Some(before_break) = lock_warning_notification {
                if next_break.duration_until() < before_break {
                    let msg = format!("locking in {}", fmt_dur(before_break));
                    notification::notify_all_users(&msg);
                }
            }
        }
        _ => (),
    }

    if state_notifications && state_changed {
        notification::notify_all_users(&msg);
    }
}

fn format_status(state: &State, idle: &Arc<Mutex<Instant>>, break_duration: Duration) -> String {
    let msg = match *state {
        State::Waiting => String::from("-"),
        State::Work { next_break } => {
            let idle = idle.lock().unwrap().elapsed();
            if idle > Duration::from_secs(30) {
                let break_dur = break_duration.saturating_sub(idle);
                let break_dur = fmt_dur(break_dur);
                format!("idle, reset in {}", break_dur)
            } else {
                let next_break = fmt_dur(next_break.duration_until());
                format!("break in {}", next_break)
            }
        }
        State::Break { next_work } => {
            format!("unlocks in {}", fmt_dur(next_work.duration_until()))
        }
    };
    msg
}

impl Status {
    pub(crate) fn new(
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
        let integrator = thread::spawn(move || {
            integrate(
                &rx,
                file_status,
                notifications,
                idle,
                break_duration,
                lock_warning_notification,
            )
        });
        let dbus_thread = thread::spawn(move || {
            dbus::maintain_blocking()
        });
        Ok(Self {
            update: tx,
            integrator: Some(integrator),
            dbus_maintain: Some(dbus_thread),
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
    }

    pub(crate) fn set_break(&mut self, next_work: Instant) {
        self.send(State::Break { next_work });
    }
}

fn fmt_mm_hh(dur: Duration) -> String {
    let mm = (dur.as_secs_f32() / 60.0).round() as u8 % 60;
    let hh = (dur.as_secs_f32() / 60.0 / 60.0).round() as u8;
    if hh == 0 {
        format!("{mm}m")
    } else {
        format!("{hh}h:{mm}m")
    }
}

fn fmt_dur(dur: Duration) -> String {
    let seconds = dur.as_secs();
    if seconds > 60 {
        fmt_mm_hh(dur)
    } else {
        return format!("{seconds}s");
    }
}
