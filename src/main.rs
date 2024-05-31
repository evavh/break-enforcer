#![feature(thread_sleep_until)]
#![feature(iter_intersperse)]
#![feature(slice_flatten)]
#![feature(io_error_more)]
#![feature(iter_collect_into)]

use clap::Parser;
use color_eyre::eyre::Context;
use color_eyre::{eyre::eyre, Section};
use tracing_subscriber::fmt::time::uptime;

mod check_inputs;
mod cli;
mod config;
mod install;
mod integration;
mod tcp_api_config;
mod run;
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
    if let sudo::RunningAs::User = sudo::check() {
        if cli.command != cli::Commands::Status {
            return Err(eyre!(concat!(
                "must run ",
                env!("CARGO_CRATE_NAME"),
                " as root user,\nExisting"
            )))
            .suppress_backtrace(true)
            .suggestion("Run using sudo");
        }
    }

    match cli.command {
        cli::Commands::Run(args) => run::run(args, cli.config_path),
        cli::Commands::Wizard => wizard::run(cli.config_path).wrap_err("Error running wizard"),
        cli::Commands::Status => print_status().wrap_err("Could not print status"),
        cli::Commands::Install(args) => {
            install::set_up(&args, cli.config_path).wrap_err("Could not install")
        }
        cli::Commands::Remove => install::tear_down().wrap_err("Could not remove"),
    }
}

fn print_status() -> color_eyre::Result<()> {
    let mut api =
        break_enforcer::Api::new().wrap_err("Error interfacing with break-enforcer instance")?;
    let msg = api.status().wrap_err("Error requesting status message")?;
    println!("{msg}");
    Ok(())
}
