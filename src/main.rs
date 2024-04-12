#![feature(thread_sleep_until)]
#![feature(iter_intersperse)]
#![feature(slice_flatten)]
#![feature(io_error_more)]

use clap::Parser;
use cli::validate_args;
use color_eyre::eyre::Context;
use color_eyre::{eyre::eyre, Section};

mod check_inputs;
mod cli;
mod config;
mod notification;
mod run;
mod watch;
mod wizard;

fn main() -> color_eyre::Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_location_section(false)
        .install()
        .expect("Only called once");
    let args = cli::Cli::parse();
    validate_args(&args)?;

    // check after args such that help can run without root
    if let sudo::RunningAs::User = sudo::check() {
        return Err(eyre!(concat!(
            "must run ",
            env!("CARGO_CRATE_NAME"),
            " as root user,\nExisting"
        )))
        .suppress_backtrace(true)
        .suggestion("Run using sudo");
    }

    match args.command {
        cli::Commands::Run {
            work_duration,
            break_duration,
            grace_duration,
        } => run::run(
            args.config_path,
            work_duration,
            break_duration,
            grace_duration,
        ),
        cli::Commands::Wizard => wizard::run(args.config_path).wrap_err("Error running wizard"),
    }
}
