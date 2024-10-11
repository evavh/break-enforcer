use std::path::PathBuf;
use std::time::{Duration, Instant};

use color_eyre::eyre::{eyre, Context};
use color_eyre::{Result, Section};

use crate::check_inputs::{InactivityTracker, InputResult, TrackResult};
use crate::cli::RunArgs;
use crate::integration::Status;
use crate::{check_inputs, watch_and_block};
use crate::{config, integration};
use std::{sync::mpsc::Receiver, thread};

pub(crate) fn run(
    RunArgs {
        work_duration,
        break_duration,
        long_break_duration,
        work_between_long_breaks,
        lock_warning,
        lock_warning_type,
        status_file,
        tcp_api,
        notifications,
    }: RunArgs,
    config_path: Option<PathBuf>,
) -> Result<()> {
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
    let (recv_any_input, recv_any_input2) =
        check_inputs::watcher(new, to_block.clone());

    let mut worked_since_long_break = Duration::from_secs(0);
    let mut inactivity_tracker =
        InactivityTracker::new(recv_any_input2, short_break_duration);
    let notify_config = integration::NotifyConfig {
        lock_warning,
        lock_warning_type,
        last_lock_warning: Instant::now(),
        state_notifications: notifications,
    };

    let idle = inactivity_tracker.idle_handle();
    let mut status = Status::new(
        status_file,
        tcp_api,
        notify_config,
        idle,
        short_break_duration,
    )
    .wrap_err("Could not setup status reporting")?;

    loop {
        status.set_waiting();

        wait_for_user_activity(&recv_any_input)
            .wrap_err("Could not wait for activity")?;
        let work_start = Instant::now();
        status.set_working(work_start + work_duration);

        let idle = match inactivity_tracker.reset_or_timeout(work_duration) {
            TrackResult::Error(e) => {
                Err(e).wrap_err("Could not track inactivity")?
            }
            TrackResult::ShouldReset => {
                worked_since_long_break +=
                    work_start.elapsed() - short_break_duration;
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

        let break_duration = match (long_break_duration, work_between_long_breaks) {
            (Some(long_break_duration), Some(work_between_long_breaks))
                // There is always some idle time before the break,
                // so we add some margin
                if worked_since_long_break + work_duration / 10
                    >= work_between_long_breaks =>
            {
                worked_since_long_break = Duration::from_secs(0);
                long_break_duration - idle
            }
            _ => {
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

fn wait_for_user_activity(
    recv_any_input: &Receiver<InputResult>,
) -> color_eyre::Result<()> {
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
        match recv_any_input.recv() {
            Err(_) => (), // device disconnected, ignore
            Ok(Err(e)) => return Err(e).wrap_err("Error with device file"),
            Ok(Ok(_)) => return Ok(()), // new event! stop blocking
        }
    }
}
