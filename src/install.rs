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

pub fn set_up(run_args: RunArgs, config_path: Option<PathBuf>) -> Result<()> {
    let to_block = config::read(config_path.clone())
        .wrap_err("Could not read devices to block from config")
        .wrap_err("Could not verify the config file is not empty")?;
    if to_block.is_empty() {
        return Err(eyre!(
            "No devices set up. The service would do nothing. Please run the wizard"
        ));
    }

    let mut args = Vec::new();
    if let Some(config_path) = config_path {
        args.push(format!("--config-path"));
        args.push(config_path.display().to_string());
    }
    args.push("run".to_string());
    args.push(format!("--work-duration"));
    args.push(fmt_dur(run_args.work_duration));
    args.push(format!("--break-duration"));
    args.push(fmt_dur(run_args.break_duration));
    args.push(format!("--grace-duration"));
    args.push(fmt_dur(run_args.grace_duration));

    let steps = install_system!()
        .current_exe()?
        .on_boot()
        .name(env!("CARGO_CRATE_NAME"))
        .description("Disables input during breaks")
        .args(args)
        .prepare_install()
        .wrap_err("Could not set up installation")?;

    tui::install::start(steps).wrap_err("Failed to run install wizard")?;
    Ok(())
}

pub fn tear_down() -> Result<()> {
    let steps = install_system!()
        .name(env!("CARGO_CRATE_NAME"))
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
