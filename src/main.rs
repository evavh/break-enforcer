#![feature(thread_sleep_until)]
#![feature(iter_intersperse)]
#![feature(slice_flatten)]
#![feature(io_error_more)]

use clap::Parser;
use color_eyre::eyre::Context;
use color_eyre::{eyre::eyre, Section};

mod check_inputs;
mod cli;
mod config;
mod install;
mod notification;
mod run;
mod watch;
mod wizard;

fn main() -> color_eyre::Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_location_section(false)
        .install()
        .expect("Only called once");
    let cli = cli::Cli::parse();

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

    match cli.command {
        cli::Commands::Run(args) => run::run(args, cli.config_path),
        cli::Commands::Wizard => wizard::run(cli.config_path).wrap_err("Error running wizard"),
        cli::Commands::Install(args) => {
            install::set_up(&args, cli.config_path).wrap_err("Could not install")
        }
        cli::Commands::Remove => install::tear_down().wrap_err("Could not remove"),
    }
}
