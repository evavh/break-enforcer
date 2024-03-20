use std::path::PathBuf;
use std::time::Duration;

use color_eyre::eyre::{Result, WrapErr};
use service_install::Install;

use crate::cli::RunArgs;

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
    let mut args = Vec::new();
    if let Some(config_path) = config_path {
        args.push(format!("--config-path"));
        args.push(config_path.display().to_string());
    }
    args.push("run".to_string());
    args.push(format!("--work_duration"));
    args.push(fmt_dur(run_args.work_duration));
    args.push(format!("--break_duration"));
    args.push(fmt_dur(run_args.break_duration));
    args.push(format!("--grace_duration"));
    args.push(fmt_dur(run_args.grace_duration));

    Install::system()
        .current_exe()?
        .on_boot()
        .name(env!("CARGO_CRATE_NAME"))
        .description("Disables input during breaks")
        .args(args)
        .install()
        .wrap_err("Could not set up installation")
}

pub fn tear_down() -> Result<()> {
    Install::system()
        .name(env!("CARGO_CRATE_NAME"))
        .remove()
        .wrap_err("Could not remove installation")
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
