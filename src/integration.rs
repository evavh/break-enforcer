use std::fmt::Display;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use break_enforcer::StateUpdate;
use color_eyre::eyre::Context;
use color_eyre::Result;

mod file_status;
use file_status::FileStatus;
use tracing::error;

use crate::cli::RunArgs;
mod notification;
pub(crate) mod tcp_api;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Waiting,
    WaitingLongReset { long_break_duration: Duration },
    Work { next_break: Instant },
    Break { next_work: Instant },
}
impl State {
    fn state_update(&self) -> StateUpdate {
        match self {
            State::Waiting => StateUpdate::Reset,
            State::Work { .. } => StateUpdate::BreakEnded,
            State::Break { .. } => StateUpdate::BreakStarted,
            State::WaitingLongReset {..} => StateUpdate::LongReset,
        }
    }
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
    /// warn if this close to locking
    pub(crate) lock_warning: Option<Duration>,
    pub(crate) lock_notify_type: Vec<NotificationType>,
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
            State::WaitingLongReset { .. }
            | State::Work { .. }
            | State::Break { .. } => Duration::from_secs(1),
        };

        let msg = format_status(&state, &idle, break_duration);
        if let Some(status) = &mut file_status {
            status.update(&msg);
        }
        if let Some(status) = &mut api_status {
            status.update_msg(&msg);
            if state_changed {
                status.update_subscribers(&state);
            }
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
            NotificationType::Audio => notification::beep_all_users()
                .wrap_err("Could not play audio notification")?,
        }
        Ok(())
    }

    pub(crate) fn check_dependency(&self) -> color_eyre::Result<()> {
        match self {
            NotificationType::System => notification::notify_available()
                .wrap_err("dependency missing for notification")?,
            NotificationType::Audio => notification::beep_available()
                .wrap_err("dependency missing for beep")?,
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
    const MARGIN: Duration = Duration::from_secs(1);
    if let State::Work { next_break } = *state {
        if let Some(warn_at) = notify.lock_warning {
            if next_break.duration_until() < warn_at {
                if notify.last_lock_warning.elapsed() > warn_at + MARGIN {
                    let msg = format!("locking in {}", fmt_dur(warn_at));
                    notify.last_lock_warning = Instant::now();
                    for notify_type in &notify.lock_notify_type {
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
        State::WaitingLongReset {
            long_break_duration,
        } => {
            let idle = idle.lock().unwrap().elapsed();
            let break_dur = long_break_duration.saturating_sub(idle);
            let break_dur = fmt_dur(break_dur);
            format!("long reset in {}", break_dur)
        }
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
        args: &RunArgs,
        idle: Arc<Mutex<Instant>>,
    ) -> Result<Self> {
        let file_status = if args.status_file {
            Some(FileStatus::new()?)
        } else {
            None
        };

        let api_status = if args.tcp_api {
            let status = tcp_api::Status::new(idle.clone());
            {
                let status = status.clone();
                let args = args.clone();
                thread::spawn(|| {
                    if let Err(e) = tcp_api::maintain(status, args) {
                        error!("failed to maintain tcp API: {e}");
                    }
                });
            }
            Some(status)
        } else {
            None
        };

        let (tx, rx) = mpsc::channel();

        let notify_config = NotifyConfig {
            lock_warning: args.lock_warning,
            lock_notify_type: args.lock_warning_type.clone(),
            last_lock_warning: Instant::now(),
            state_notifications: args.notifications,
        };

        let break_duration = args.break_duration;
        let integrator = thread::spawn(move || {
            integrate(
                &rx,
                file_status,
                api_status,
                idle,
                break_duration,
                notify_config,
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

    pub(crate) fn set_waiting_long_reset(
        &mut self,
        long_break_duration: Duration,
    ) {
        self.send(State::WaitingLongReset {
            long_break_duration,
        });
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
