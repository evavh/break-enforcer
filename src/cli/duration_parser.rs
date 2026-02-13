use std::time::Duration;
use std::num::ParseFloatError;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Could not parse the seconds, input: {1}")]
    Second(#[source] ParseFloatError, String),
    #[error("Could not parse the minutes, input: {1}")]
    Minute(#[source] ParseFloatError, String),
    #[error("Could not parse the hours, input: {1}")]
    Hour(#[source] ParseFloatError, String),
    #[error("Durations need a suffix like s, m or h or one separator `:`")]
    NoColonOrUnit(String),
}

pub(crate) fn second_err(e: ParseFloatError, s: &str) -> ParseError {
    ParseError::Second(e, s.to_owned())
}

pub(crate) fn minute_err(e: ParseFloatError, s: &str) -> ParseError {
    ParseError::Minute(e, s.to_owned())
}

pub(crate) fn hour_err(e: ParseFloatError, s: &str) -> ParseError {
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
    seconds += 60.0
        * minutes.parse::<f32>().map_err(|e| minute_err(e, minutes))?;
    if hours.is_empty() {
        return Ok(seconds);
    };
    seconds += 60.0
        * 60.0
        * hours.parse::<f32>().map_err(|e| hour_err(e, hours))?;
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
pub(crate) mod test {
    use super::*;

    #[test]
    fn test_colon_duration() {
        assert_eq!(parse_colon_duration("10:00").unwrap(), 60. * 10.);
        assert_eq!(parse_colon_duration("07:00").unwrap(), 60. * 7.);
    }
}

