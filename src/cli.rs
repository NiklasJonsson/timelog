use std::process::ExitCode;

use chrono::{Local, NaiveTime, ParseResult};

use crate::timelogger::TimeLogger;

pub fn parse_time_arg(s: &str) -> ParseResult<NaiveTime> {
    // %R = %H:%M
    // %H: hour, two digits
    // %M: minute, two digits
    NaiveTime::parse_from_str(s, "%R")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H.%M"))
        .or_else(|_| NaiveTime::parse_from_str(s, "%H"))
}

pub fn get_time(s: Option<String>) -> ParseResult<NaiveTime> {
    match s {
        Some(x) => parse_time_arg(&x),
        None => Ok(Local::now().time()),
    }
}

// TODO: There is some duplication between start and end. Fix this.
pub fn start(tl: &mut TimeLogger, time: Option<String>) -> ExitCode {
    let time = match get_time(time) {
        Ok(t) => t,
        Err(e) => {
            println!("Unable to update timelog: {}", e);
            return ExitCode::FAILURE;
        }
    };

    let entry = tl.log_start(Local::today().naive_local(), time);

    if let Err(e) = tl.save() {
        println!("Failed to save to logfile: {}", e);
        return ExitCode::FAILURE;
    }

    println!("Logged: starting {} at {}", entry.ty(), entry.start().expect("The start value was just set"));

    ExitCode::SUCCESS
}

pub fn end(tl: &mut TimeLogger, time: Option<String>) -> ExitCode {
    let time = match get_time(time) {
        Ok(t) => t,
        Err(e) => {
            println!("Unable to update timelog: {}", e);
            return ExitCode::FAILURE;
        }
    };

    let entry = tl.log_end(Local::today().naive_local(), time);

    if let Err(e) = tl.save() {
        println!("Failed to save to logfile: {}", e);
        return ExitCode::FAILURE;
    }

    println!("Logged: ending {} at {}", entry.ty(), entry.end().expect("The end value was just set"));

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;
    #[test]
    fn parse_time() {
        assert_eq!(
            parse_time_arg(&String::from("03:00")),
            Ok(NaiveTime::from_hms(3, 0, 0))
        );
        assert_eq!(
            parse_time_arg(&String::from("03.00")),
            Ok(NaiveTime::from_hms(3, 0, 0))
        );
        assert_eq!(
            parse_time_arg(&String::from("3.00")),
            Ok(NaiveTime::from_hms(3, 0, 0))
        );
        assert_eq!(
            parse_time_arg(&String::from("3.0")),
            Ok(NaiveTime::from_hms(3, 0, 0))
        );
        assert_eq!(
            parse_time_arg(&String::from("03.0")),
            Ok(NaiveTime::from_hms(3, 0, 0))
        );
        assert_eq!(
            parse_time_arg(&String::from("3.00")),
            Ok(NaiveTime::from_hms(3, 0, 0))
        );
    }
}
