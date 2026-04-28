#![feature(iter_collect_into)]

use std::time::{Duration, Instant};

use clap::Parser;
use color_eyre::eyre::Context;
use color_eyre::{Section, eyre::eyre};
use tracing_subscriber::fmt::time::uptime;

use crate::run::Time;

mod check_inputs;
mod cli;
mod config;
mod install;
mod integration;
mod mockies;
mod run;
mod status;
mod tcp_api_config;
mod watch_and_block;
mod wizard;

fn main() -> color_eyre::Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_location_section(false)
        .install()
        .expect("Only called once");

    let cli = cli::Cli::parse();

    let trace_level = if cli.verbose {
        tracing::Level::TRACE
    } else {
        tracing::Level::WARN
    };

    tracing_subscriber::fmt()
        .with_max_level(trace_level)
        .with_file(false)
        .with_target(false)
        .with_timer(uptime())
        .init();

    // check after args such that help can run without root
    if let sudo::RunningAs::User = sudo::check()
        && cli.command.needs_sudo()
    {
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
        cli::Commands::Wizard => {
            wizard::run(cli.config_path).wrap_err("Error running wizard")
        }
        cli::Commands::Status(args) => {
            status::run(args).wrap_err("Could not print status")
        }
        cli::Commands::Install(args) => install::set_up(&args, cli.config_path)
            .wrap_err("Could not install"),
        cli::Commands::Remove => {
            install::tear_down().wrap_err("Could not remove")
        }
    }
}

trait InstantExt {
    fn duration_until(&self) -> Duration;
    fn in_the_future(&self) -> bool;
    #[cfg(test)]
    fn in_the_past(&self) -> bool;
}

impl<T: Time> InstantExt for T {
    fn duration_until(&self) -> Duration {
        self.saturating_duration_since(T::now())
    }
    fn in_the_future(&self) -> bool {
        Instant::now().elapsed().is_zero()
    }
    #[cfg(test)]
    fn in_the_past(&self) -> bool {
        Instant::now().elapsed() > Duration::ZERO
    }
}
