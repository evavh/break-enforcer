use clap::{Args, Parser, Subcommand};
use std::num::ParseFloatError;
use std::path::PathBuf;
use std::time::Duration;

use crate::integration::NotificationType;

#[allow(clippy::struct_field_names)]
#[derive(Debug, Args, PartialEq, Eq)]
pub struct RunArgs {
    /// Period after which input will be disabled.  
    /// Note: run help command to see the duration format.
    #[arg(short, long, value_name = "duration", value_parser = parse_duration)]
    pub work_duration: Duration,
    /// Length of the breaks, after this period input is resumed.
    /// Note: run help command to see the duration format.
    #[arg(short, long, value_name = "duration", value_parser = parse_duration)]
    pub break_duration: Duration,
    /// Optional takes a duration, if set sends a notification ahead of the break.
    /// Note: run help command to see the duration format.
    #[arg(short, long, value_name = "duration", value_parser = parse_duration)]
    pub lock_warning: Option<Duration>,
    /// Type of notification to get as lock warning.
    /// - For audio you need aplay installed.
    /// - For system you need notify-send installed.
    #[arg(short('a'), long, value_enum)]
    pub lock_warning_type: Vec<NotificationType>,
    /// Enable the tcp api. Enables the `Status` command and other apps
    /// to interface using the break-enforcer library. The API only
    /// accepts connections from the same system.
    #[arg(short, long)]
    pub tcp_api: bool,
    /// Enable the status file. It contains a string describing the time till
    /// the next break, the time till the current break is over or that the user
    /// is idle. The file is located at `/var/run/break_enforcer` and is called
    /// `status.txt`
    #[arg(short, long)]
    pub status_file: bool,
    /// verbose notifications. Sends notifications when:
    /// the break begins, a work session begins, we are waiting for input
    #[arg(short, long)]
    pub notifications: bool,
}

#[allow(clippy::struct_field_names)]
#[derive(Debug, Args, PartialEq, Eq)]
pub struct StatusArgs {
    /// Instead of printing the status once print it every `update` period
    #[arg(short, long, value_name = "duration", value_parser = parse_duration)]
    pub update_period: Option<Duration>,
    /// Output the status as json like this: {'msg': 'break in 5m'}
    #[arg(short = 'j', long)]
    pub use_json: bool,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum Commands {
    /// Periodically block devices in config (setup using wizard).
    Run(#[command(flatten)] RunArgs),
    /// Pick the devices to block and write them to a config file.
    /// (Interactive UI)
    Wizard,
    /// Moves the executable to a suitable location and set up a service.
    Install(#[command(flatten)] RunArgs),
    /// Removed the installed service and executable.
    Remove,
    /// Prints a status line describing the time till the next break,
    /// the time till the current break is over or that the user is idle.
    Status(#[command(flatten)] StatusArgs),
}

impl Commands {
    pub fn needs_sudo(&self) -> bool {
        !matches!(self, Commands::Status { .. })
    }
}

/// Disables specified input devices during breaks. The period between breaks,
/// length of the breaks and time before getting a warning can all be specified.
///
/// Durations can be passed in two formats:
///  - <amount><unit>, for example: 32m
///    unit is one of h,m and s
///  - hh:mm:ss, where hh and mm are optional however you
///    do need at least one `:`
///    * example: 1:30:15
///         one and a halve hour and 15 seconds
///    * example: 10:40
///         ten minutes and 40 seconds
///
#[derive(Parser, Debug)]
#[command(version, about, verbatim_doc_comment)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    /// Path to create/read/update list of devices to/from
    /// Default: /etc/break-enforcer.ron
    #[arg(short, long)]
    #[arg(verbatim_doc_comment)]
    pub config_path: Option<PathBuf>,
    /// Print many traces and logs
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Could not parse the seconds, input: {1}")]
    Second(#[source] ParseFloatError, String),
    #[error("Could not parse the minutes, input: {1}")]
    Minute(#[source] ParseFloatError, String),
    #[error("Could not parse the hours, input: {1}")]
    Hour(#[source] ParseFloatError, String),
    #[error("Durations need a suffix or one `:`")]
    NoColonOrUnit(String),
}

fn second_err(e: ParseFloatError, s: &str) -> ParseError {
    ParseError::Second(e, s.to_owned())
}
fn minute_err(e: ParseFloatError, s: &str) -> ParseError {
    ParseError::Minute(e, s.to_owned())
}
fn hour_err(e: ParseFloatError, s: &str) -> ParseError {
    ParseError::Hour(e, s.to_owned())
}

/// Parses a string in format
///     hh:mm:ss,
///     mm:ss,
///     :ss,
pub(crate) fn parse_colon_duration(arg: &str) -> Result<f32, ParseError> {
    let Some((rest, seconds)) = arg.rsplit_once(':') else {
        return Err(ParseError::NoColonOrUnit(arg.to_string()));
    };

    let mut seconds = seconds.parse().map_err(|e| second_err(e, arg))?;
    if rest.is_empty() {
        return Ok(seconds);
    }

    let Some((hours, minutes)) = rest.rsplit_once(':') else {
        let minutes: f32 = rest.parse().map_err(|e| minute_err(e, arg))?;
        seconds += 60.0 * minutes;
        return Ok(seconds);
    };
    seconds += 60.0 * minutes.parse::<f32>().map_err(|e| minute_err(e, minutes))?;
    if hours.is_empty() {
        return Ok(seconds);
    };
    seconds += 60.0 * 60.0 * hours.parse::<f32>().map_err(|e| hour_err(e, hours))?;
    Ok(seconds)
}

/// Parse a string in two different formats to a `Duration`. The formats are:
///  - 10h
///  - 15m
///  - 30s
///  - hh:mm:ss,
///  - mm:ss,
///  - :ss,
pub(crate) fn parse_duration(arg: &str) -> Result<Duration, ParseError> {
    let seconds = if let Some(hours) = arg.strip_suffix('h') {
        60. * 60. * hours.parse::<f32>().map_err(|e| hour_err(e, hours))?
    } else if let Some(minutes) = arg.strip_suffix('m') {
        60. * minutes.parse::<f32>().map_err(|e| minute_err(e, minutes))?
    } else if let Some(seconds) = arg.strip_suffix('s') {
        seconds.parse::<f32>().map_err(|e| second_err(e, seconds))?
    } else {
        parse_colon_duration(arg)?
    };
    Ok(std::time::Duration::from_secs_f32(seconds))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_colon_duration() {
        assert_eq!(parse_colon_duration("10:00").unwrap(), 60. * 10.);
        assert_eq!(parse_colon_duration("07:00").unwrap(), 60. * 7.);
    }
}
