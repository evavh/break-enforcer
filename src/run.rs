use std::path::PathBuf;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use color_eyre::eyre::{eyre, Context};
use color_eyre::{Result, Section};
use tracing::trace;

use crate::check_inputs::{InactivityTracker, InputResult, TrackResult};
use crate::cli::RunArgs;
use crate::config;
use crate::integration::Status;
use crate::{check_inputs, watch_and_block};
use std::{sync::mpsc::Receiver, thread};

pub(crate) fn run(
    args: RunArgs,
    config_path: Option<PathBuf>,
) -> Result<()> {
    // TODO: use args.<member> instead
    let RunArgs {
        work_duration,
        break_duration,
        long_break_duration,
        work_between_long_breaks,
        lock_warning: _,
        ref lock_warning_type,
        status_file: _,
        tcp_api: _,
        notifications: _,
    } = args;

    trace!("Long break: {long_break_duration:?}");
    trace!("Work between: {work_between_long_breaks:?}");

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
    for warning_type in lock_warning_type {
        warning_type
            .check_dependency()
            .wrap_err("Can not provide configured warning/notification")?;
    }

    let (recv_any_input, recv_any_input2) =
        check_inputs::watcher(new, to_block.clone());

    let mut worked_since_long_break = Duration::from_secs(0);
    let mut inactivity_tracker =
        InactivityTracker::new(recv_any_input2, short_break_duration);

    let idle = inactivity_tracker.idle_handle();
    let mut status = Status::new(&args, idle)
        .wrap_err("Could not setup status reporting")?;

    loop {
        if worked_since_long_break > Duration::from_secs(0) {
            if let Some(long_break_duration) = long_break_duration {
                status.set_waiting_long_reset(long_break_duration);
                match wait_for_user_activity(
                    &recv_any_input,
                    long_break_duration - short_break_duration,
                )
                .wrap_err("Could not wait for activity")?
                {
                    IdleResult::Activity => (),
                    IdleResult::Timeout => {
                        trace!("Idle > long break, resetting total work time");
                        worked_since_long_break = Duration::from_secs(0);
                        continue;
                    }
                }
            }
        } else {
            status.set_waiting();
            wait_for_user_activity(&recv_any_input, Duration::MAX)
                .wrap_err("Could not wait for activity")?;
        }

        let work_start = Instant::now();
        status.set_working(work_start + work_duration);

        let idle = match inactivity_tracker.reset_or_timeout(work_duration)
        {
            TrackResult::Error(e) => {
                Err(e).wrap_err("Could not track inactivity")?
            }
            TrackResult::ShouldReset => {
                worked_since_long_break +=
                    work_start.elapsed().saturating_sub(short_break_duration);
                continue;
            }
            TrackResult::ShouldBreak { user_idle } => {
                worked_since_long_break += work_start.elapsed() - user_idle;
                user_idle
            }
        };

        let mut locks = Vec::new();
        for device_id in to_block.iter().cloned() {
            locks.push(
                online_devices
                    .lock(device_id)
                    .wrap_err("failed to lock one of the inputs")?,
            );
        }

        trace!("Worked since long break: {worked_since_long_break:?}");
        let break_duration = match (long_break_duration, work_between_long_breaks) {
            (Some(long_break_duration), Some(work_between_long_breaks))
                // There is always some idle time before the break,
                // so we add some margin
                if worked_since_long_break + work_duration / 10
                    >= work_between_long_breaks =>
            {
                trace!("Starting long break, resetting total work time");
                worked_since_long_break = Duration::from_secs(0);
                long_break_duration - idle
            }
            _ => {
                trace!("Starting short break");
                short_break_duration - idle
            }
        };

        status.set_break(Instant::now() + break_duration);
        thread::sleep(break_duration);

        for lock in locks {
            lock.unlock()?;
        }
    }
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
