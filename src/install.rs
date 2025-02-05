use std::path::PathBuf;
use std::time::Duration;

use color_eyre::eyre::{eyre, Context, Result};
use service_install::{install_system, tui};

use crate::cli::RunArgs;
use crate::config;

fn fmt_dur(dur: Duration) -> String {
    let ss = dur.as_secs() % 60;
    let mm = (dur.as_secs() / 60) % 60;
    if mm == 0 {
        return format!("{ss}s");
    }
    let hh = dur.as_secs() / 60 / 60;
    if hh == 0 {
        format!("{mm:02}:{ss:02}")
    } else {
        format!("{hh:02}:{mm:02}:{ss:02}")
    }
}

pub fn set_up(run_args: &RunArgs, config_path: Option<PathBuf>) -> Result<()> {
    let to_block = config::read(config_path.clone())
        .wrap_err("Could not read devices to block from config")
        .wrap_err("Could not verify the config file is not empty")?;
    if to_block.is_empty() {
        return Err(eyre!(
            "No devices set up. The service would do nothing. Please run the wizard"
        ));
    }
    for warning_type in &run_args.lock_warning_type {
        warning_type
            .check_dependency()
            .wrap_err("Can not provide configured warning/notification")?;
    }

    let mut args = Vec::new();
    if let Some(config_path) = config_path {
        args.push("--config-path".to_string());
        args.push(config_path.display().to_string());
    }
    args.push("run".to_string());
    args.push("--work-duration".to_string());
    args.push(fmt_dur(run_args.work_duration));
    args.push("--break-duration".to_string());
    args.push(fmt_dur(run_args.break_duration));
    if let Some(warn_duration) = run_args.lock_warning {
        args.push("--lock-warning".to_string());
        args.push(fmt_dur(warn_duration));
    }
    for warn_type in &run_args.lock_warning_type {
        args.push("--lock-warning-type".to_string());
        args.push(warn_type.to_string());
    }
    if run_args.status_file {
        args.push("--status-file".to_string());
    }
    if run_args.tcp_api {
        args.push("--tcp-api".to_string());
    }

    let name = env!("CARGO_CRATE_NAME").replace("_", "-");
    let steps = install_system!()
        .current_exe()?
        .on_boot()
        .service_name(name)
        .description("Disables input during breaks")
        .args(args)
        .overwrite_existing(true)
        .prepare_install()
        .wrap_err("Could not set up installation")?;

    tui::install::start(steps, true)
        .wrap_err("Failed to run install wizard")?;
    Ok(())
}

pub fn tear_down() -> Result<()> {
    let steps = install_system!()
        .service_name(env!("CARGO_CRATE_NAME"))
        .prepare_remove()
        .wrap_err("Could not remove installation")?;

    tui::removal::start(steps).wrap_err("Failed to run removal wizard")?;
    Ok(())
}

#[test]
fn test_fmt_dur() {
    assert_eq!(
        &fmt_dur(Duration::from_secs(8 * 60 * 60 + 4 * 60 + 5)),
        "08:04:05"
    );

    assert_eq!(&fmt_dur(Duration::from_secs(0)), "0s");
    assert_eq!(&fmt_dur(Duration::from_secs(61)), "01:01");
}
