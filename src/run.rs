use std::path::PathBuf;
use std::time::Instant;

use color_eyre::eyre::{eyre, Context};
use color_eyre::{Result, Section};

use crate::check_inputs::InputResult;
use crate::cli::RunArgs;
use crate::integration::Status;
use crate::{check_inputs, watch_and_block};
use crate::{check_inputs::inactivity_watcher, config};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, RecvTimeoutError},
        Arc,
    },
    thread,
};

pub(crate) fn run(
    RunArgs {
        work_duration,
        break_duration,
        lock_warning,
        status_file,
        notifications,
    }: RunArgs,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let (online_devices, new) = watch_and_block::devices();

    let to_block =
        config::read(config_path).wrap_err("Could not read devices to block from config")?;
    if to_block.is_empty() {
        return Err(eyre!(
            "No config, do not know what to block. Please run the wizard. \nExiting"
        ))
        .suppress_backtrace(true)
        .suggestion("Run the wizard")
        .suggestion("Maybe you have a (wrong) custom location set?");
    }
    let (recv_any_input, recv_any_input2) = check_inputs::watcher(new, to_block.clone());

    let (break_skip_sender, break_skip_receiver) = channel();
    let (work_start_sender, work_start_receiver) = channel();
    let break_skip_is_sent = Arc::new(AtomicBool::new(false));

    {
        let break_skip_is_sent = break_skip_is_sent.clone();

        thread::spawn(move || {
            inactivity_watcher(
                &work_start_receiver,
                &break_skip_sender,
                &break_skip_is_sent,
                &recv_any_input2,
                break_duration,
            );
        });
    }

    let mut status = Status::new(status_file, notifications, lock_warning)
        .wrap_err("Could not setup status reporting")?;

    loop {
        status.set_waiting();

        block_on_new_input(&recv_any_input).wrap_err("Could not block till new input")?;
        work_start_sender.send(true).unwrap();
        status.set_working(Instant::now() + work_duration);

        match break_skip_receiver.recv_timeout(work_duration) {
            Ok(_) => {
                status.set_waiting();
                block_on_new_input(&recv_any_input).wrap_err("Could not block till new input")?;
                break_skip_is_sent.store(false, Ordering::Release);
                continue;
            }
            Err(RecvTimeoutError::Timeout) => (),
            Err(e) => panic!("Unexpected error: {e}"),
        }

        let mut locks = Vec::new();

        for device_id in to_block.iter().cloned() {
            locks.push(
                online_devices
                    .lock(device_id)
                    .wrap_err("failed to lock one of the inputs")?,
            );
        }

        status.set_on_break(Instant::now() + break_duration);
        thread::sleep(break_duration);

        for lock in locks {
            lock.unlock()?;
        }
    }
}

fn block_on_new_input(recv_any_input: &Receiver<InputResult>) -> color_eyre::Result<()> {
    loop {
        match recv_any_input.try_recv() {
            Err(_) => break,
            Ok(Err(e)) => return Err(e).wrap_err("Error with device file"),
            Ok(Ok(_)) => (), // old event, ignore
        }
    }

    #[allow(clippy::match_same_arms)]
    match recv_any_input.recv() {
        Err(_) => Ok(()), // device disconnected
        Ok(Err(e)) => Err(e).wrap_err("Error with device file"),
        Ok(Ok(_)) => Ok(()), // new event! stop blocking
    }
}
