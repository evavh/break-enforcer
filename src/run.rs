use std::iter;
use std::path::PathBuf;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use color_eyre::eyre::{Context, eyre};
use color_eyre::{Result, Section};

use crate::check_inputs::{InactivityTracker, InputResult, TrackResult};
use crate::cli::RunArgs;
use crate::integration::Status;
use crate::run::state_machine::{Devices, UserTracking};
use crate::{InstantExt, config};
use crate::{check_inputs, watch_and_block};
use std::sync::mpsc::Receiver;

mod state_machine;

pub(crate) fn run(args: RunArgs, config_path: Option<PathBuf>) -> Result<()> {
    let RunArgs {
        work_duration,
        break_duration,
        long_break_duration,
        work_between_long_breaks,
        break_start_notify: ref lock_warning,
        break_end_notify: ref lock_release,
        ..
    } = args;

    let short_break_duration = break_duration;
    if let Some(long_break_duration) = long_break_duration {
        assert!(long_break_duration > short_break_duration);
    }

    let (online_devices, new) = watch_and_block::devices();

    let to_block = config::read(config_path)
        .wrap_err("Could not read devices to block from config")?;
    if to_block.is_empty() {
        return Err(eyre!(
            "No config, do not know what to block. Please run the wizard. \nExiting"
        ))
        .suppress_backtrace(true)
        .suggestion("Run the wizard")
        .suggestion("Maybe you have a (wrong) custom location set?");
    }

    for warning_type in lock_warning.iter().chain(lock_release.iter()) {
        warning_type
            .check_dependency()
            .wrap_err("Can not provide configured warning/notification")?;
    }

    let (recv_any_input, recv_any_input2) =
        check_inputs::watcher(new, to_block.clone());

    let inactivity_tracker =
        InactivityTracker::new(recv_any_input2, short_break_duration);
    let idle = inactivity_tracker.idle_handle();

    let tracking = RealUserTracking {
        watcher: inactivity_tracker,
        by: recv_any_input,
    };

    let devices = RealDevices {
        list: to_block,
        online_devices,
    };

    let mut breaks = iter::once(Break {
        duration: short_break_duration,
        between: work_duration,
        next_at: Instant::now(),
    })
    .chain(long_break_duration.zip(work_between_long_breaks).map(
        |(duration, between)| Break {
            duration,
            between,
            next_at: Instant::now(),
        },
    ))
    .collect::<Vec<_>>();

    let mut status = Status::new(&args, idle)
        .wrap_err("Could not setup status reporting")?;

    state_machine::run(tracking, devices, &mut breaks, |change| match change {
        NewState::Idle => status.set_waiting(),
        NewState::Running { next } => status.set_working(next.next_at),
        NewState::Break { current } => {
            status.set_break(Instant::now() + current.duration)
        }
    })
}

struct RealDevices {
    list: Vec<crate::config::InputFilter>,
    online_devices: crate::watch_and_block::OnlineDevices,
}

impl Devices for RealDevices {
    type LockGuard = crate::watch_and_block::LockGuard;
    fn lock(&self) -> Result<Vec<crate::watch_and_block::LockGuard>> {
        self.list
            .iter()
            .map(|device_id| {
                self.online_devices
                    .lock(device_id.clone())
                    .wrap_err("failed to lock one of the inputs")
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
struct Break {
    /// the break takes this long, locking input devices for this time
    duration: Duration,
    /// time between these breaks, also known as the work period
    between: Duration,
    /// when the next occurrence of this break will happen
    /// this is delayed if idle time occurs
    next_at: Instant,
}

#[derive(Debug, Clone, Copy)]
enum NewState {
    Idle,
    Running { next: Break },
    Break { current: Break },
}

struct RealUserTracking {
    watcher: InactivityTracker,
    by: Receiver<InputResult>,
}

impl UserTracking for RealUserTracking {
    fn idle_until_input(&self) -> Result<Duration> {
        let before_activity = Instant::now();
        match wait_for_user_activity(&self.by, Duration::MAX)
            .wrap_err("Could not wait for activity")?
        {
            IdleResult::Activity => Ok(before_activity.elapsed()),
            IdleResult::Timeout => unreachable!(),
        }
    }

    fn reset_or_time_for_break(
        &mut self,
        Break {
            next_at: next_break,
            between,
            ..
        }: &Break,
    ) -> Result<ResetOrBreak> {
        while next_break.in_the_future() {
            match was_idle_or_timeout(&mut self.watcher, next_break)? {
                IdleOrTimeout::Timeout => (),
                IdleOrTimeout::IdleFor(idle_time) if idle_time > *between => {
                    return Ok(ResetOrBreak::Reset { idle_time });
                }
                IdleOrTimeout::IdleFor(_) => (),
            }
        }
        Ok(ResetOrBreak::Break)
    }
}

fn was_idle_or_timeout(
    watcher: &mut InactivityTracker,
    next_break: &Instant,
) -> Result<IdleOrTimeout> {
    match watcher.reset_or_timeout(next_break.duration_until()) {
        TrackResult::ShouldReset => Ok(IdleOrTimeout::Timeout),
        TrackResult::ShouldBreak { user_idle } => {
            Ok(IdleOrTimeout::IdleFor(user_idle))
        }
        TrackResult::Error(report) => Err(report),
    }
}

enum IdleOrTimeout {
    Timeout,
    IdleFor(Duration),
}

enum ResetOrBreak {
    Reset { idle_time: Duration },
    Break,
}

enum IdleResult {
    Activity,
    Timeout,
}

fn wait_for_user_activity(
    recv_any_input: &Receiver<InputResult>,
    timeout: Duration,
) -> color_eyre::Result<IdleResult> {
    loop {
        // clear old events
        match recv_any_input.try_recv() {
            Err(_) => break,
            Ok(Err(e)) => return Err(e).wrap_err("Error with device file"),
            Ok(Ok(_)) => (), // old event, ignore
        }
    }

    loop {
        #[allow(clippy::match_same_arms)]
        match recv_any_input.recv_timeout(timeout) {
            Ok(Err(e)) => return Err(e).wrap_err("Error with device file"),
            Ok(Ok(_)) => return Ok(IdleResult::Activity), // new event! stop blocking
            Err(RecvTimeoutError::Timeout) => return Ok(IdleResult::Timeout),
            Err(_) => (), // device disconnected, ignore
        }
    }
}
