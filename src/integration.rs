use std::fmt::Display;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use color_eyre::eyre::Context;
use color_eyre::Result;

mod file_status;
use file_status::FileStatus;
use tracing::error;
mod notification;
pub(crate) mod tcp_api;

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
}

pub(crate) struct NotifyConfig {
    pub(crate) lock_warning: Option<Duration>,
    pub(crate) lock_warning_type: Vec<NotificationType>,
    pub(crate) last_lock_warning: Instant,
    pub(crate) state_notifications: bool,
}

fn integrate(
    rx: &mpsc::Receiver<State>,
    mut file_status: Option<FileStatus>,
    mut api_status: Option<tcp_api::Status>,
    idle: Arc<Mutex<Instant>>,
    break_duration: Duration,
    mut notify: NotifyConfig,
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
        if let Some(status) = &mut file_status {
            status.update(&msg);
        }
        if let Some(status) = &mut api_status {
            status.update_msg(&msg);
        }
        notify_if_needed(&state, &mut notify, state_changed, msg);
    }
}

#[derive(Debug, Clone, clap::ValueEnum, Eq, PartialEq)]
pub(crate) enum NotificationType {
    System,
    Audio,
}

impl Display for NotificationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationType::System => f.write_str("system"),
            NotificationType::Audio => f.write_str("audio"),
        }
    }
}

impl NotificationType {
    fn notify(&self, msg: &str) -> color_eyre::Result<()> {
        match self {
            NotificationType::System => notification::notify(msg)
                .wrap_err("Could not send system notification")?,
            NotificationType::Audio => notification::beep()
                .wrap_err("Could not play audio notification")?,
        }
        Ok(())
    }
}

fn notify_if_needed(
    state: &State,
    notify: &mut NotifyConfig,
    state_changed: bool,
    msg: String,
) {
    if let State::Work { next_break } = *state {
        if let Some(before_break) = notify.lock_warning {
            if next_break.duration_until() < before_break {
                if notify.last_lock_warning.elapsed() > before_break {
                    let msg = format!("locking in {}", fmt_dur(before_break));
                    for notify_type in &notify.lock_warning_type {
                        if let Err(report) = notify_type.notify(&msg) {
                            error!("Failed to send lock warning: {report}")
                        }
                    }
                }
            }
        }
    }

    if notify.state_notifications && state_changed {
        if let Err(report) = notification::notify(&msg) {
            error!("Failed to send state change notification: {report}")
        }
    }
}

fn format_status(
    state: &State,
    idle: &Arc<Mutex<Instant>>,
    break_duration: Duration,
) -> String {
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
        tcp_api_integration: bool,
        notify: NotifyConfig,
        idle: Arc<Mutex<Instant>>,
        break_duration: Duration,
    ) -> Result<Self> {
        let file_status = if file_integration {
            Some(FileStatus::new()?)
        } else {
            None
        };

        let api_status = if tcp_api_integration {
            let status = tcp_api::Status::new(idle.clone());
            {
                let status = status.clone();
                thread::spawn(|| {
                    if let Err(e) = tcp_api::maintain(status) {
                        error!("failed to maintain tcp API: {e}");
                    }
                });
            }
            Some(status)
        } else {
            None
        };

        let (tx, rx) = mpsc::channel();
        let integrator = thread::spawn(move || {
            integrate(
                &rx,
                file_status,
                api_status,
                idle,
                break_duration,
                notify,
            )
        });

        Ok(Self {
            update: tx,
            integrator: Some(integrator),
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
                .expect(
                    "The integrator thread returned an error, it should not",
                );
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
        format!("{seconds}s")
    }
}
