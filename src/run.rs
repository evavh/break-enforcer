use std::{path::PathBuf, time::Duration};

use color_eyre::eyre::{eyre, Context};
use color_eyre::{Result, Section};

use crate::check_inputs::InputResult;
use crate::notification::notify_all_users;
use crate::{check_inputs, watch};
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
    config_path: Option<PathBuf>,
    work_duration: Duration,
    break_duration: Duration,
    grace_duration: Duration,
) -> Result<()> {
    let (online_devices, new) = watch::devices();

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

    loop {
        notify_all_users("Waiting for input to start work timer...");
        block_on_new_input(&recv_any_input).wrap_err("Could not block till new input")?;
        notify_all_users(&format!("Starting work timer for {work_duration:?}"));
        work_start_sender.send(true).unwrap();
        match break_skip_receiver.recv_timeout(work_duration - grace_duration) {
            Ok(_) => {
                notify_all_users("No input for breaktime");
                block_on_new_input(&recv_any_input).wrap_err("Could not block till new input")?;
                break_skip_is_sent.store(false, Ordering::Release);
                continue;
            }
            Err(RecvTimeoutError::Timeout) => (),
            Err(e) => panic!("Unexpected error: {e}"),
        }

        notify_all_users(&format!("Locking in {grace_duration:?}!"));
        thread::sleep(grace_duration);

        let mut locks = Vec::new();

        for device_id in to_block.iter().cloned() {
            locks.push(
                online_devices
                    .lock(device_id)
                    .wrap_err("failed to lock one of the inputs")?,
            );
        }

        notify_all_users(&format!("Starting break timer for {break_duration:?}"));
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
