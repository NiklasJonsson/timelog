#[macro_use]
extern crate serde_derive;
extern crate docopt;
extern crate chrono;

mod timelog;

use docopt::Docopt;
use chrono::prelude::*;
use chrono::NaiveTime;
use chrono::Duration;
use chrono::ParseResult;
use timelog::TimeLogger;
use timelog::TimeLogResult;
use timelog::TimeLogError;

const USAGE: &'static str = "
Timelog

Usage:
  timelog start [<time>]
  timelog end [<time>]
  timelog break [<duration>]
  timelog month
  timelog week
  timelog day
  timelog
  timelog (-h | --help)

Options:
  -h --help     Show this screen.
";

// TODO: Nice features:
// How many hours have i Worked this week?
// Remaing hours / remaining days (avg)
// Close to optimal schedule? Store optimal schedule in str/file (maybe .timelog/.schedule.txt?)
// (calculate by hand) and then print stats on how close I am.

#[derive(Debug, Deserialize)]
struct Args {
	cmd_start: bool,
	cmd_end: bool,
	cmd_break: bool,
    cmd_month: bool,
    cmd_week: bool,
    cmd_day: bool,
	arg_time: Option<String>,
	arg_duration: Option<String>,
}

fn parse_time_arg(s: &String) -> ParseResult<NaiveTime> {
    // %R = %H:%M
    // %H: hour, two digits
    // %M: minute, two digits
    NaiveTime::parse_from_str(s, "%R")
}

fn get_time(s: Option<String>) -> ParseResult<NaiveTime> {
    match s {
        Some(x) => parse_time_arg(&x),
        None => Ok(Local::now().time()),
    }
}

fn get_duration(s: Option<String>) ->  TimeLogResult<Duration> {
    match s {
        None => Err(TimeLogError::InvalidInputError(String::from("Can't parse None value"))),
        Some(x) => timelog::parse_duration(x.as_str()),
    }
}

fn real_main() -> i32 {
	let args: Args = Docopt::new(USAGE)
		.and_then(|d| d.deserialize())
		.unwrap_or_else(|e| e.exit());

    let mut tl = match TimeLogger::current_month() {
      Ok(x) => x,
      Err(e) => {
        println!("ERROR: Could not create Timelogger instance: {}", e);
        return 1;
      }
    };

    if args.cmd_start {
        let time = match get_time(args.arg_time) {
            Ok(t) => t,
            Err(e) => {
                println!("Unable to update timelog: {}", e);
                return 1;
            },
        };
        tl.log_start(time);
    } else if args.cmd_end {
        let time = match get_time(args.arg_time) {
            Ok(t) => t,
            Err(e) => {
                println!("Unable to update timelog: {}", e);
                return 1;
            },
        };
        tl.log_end(time);
    } else if args.cmd_break {
        let dur = match get_duration(args.arg_duration) {
            Ok(d) => d,
            Err(e) => {
                println!("Unable to update timelog: {}", e);
                return 1;
            },
        };
        tl.log_break(dur);
    } else if args.cmd_month {
        println!("{} hrs left of {} this month", tl.hours_left_this_month(), tl.total_hours_this_month());
    } else if args.cmd_week {
        // TODO: Print if end of week or beginning of week is not the same month
        // TODO: Read from other file if possible
        let time = tl.time_left_this_week();
        println!("{};{} left this week", time.num_hours(), time.num_minutes() % 60);
    } else if args.cmd_day {
        let diff = match tl.time_worked_today() {
            Ok(x) => x,
            Err(e) => {
                println!("Couldn't calculate time worked today: {}", e);
                return 1;
            },
        };
        let hours = diff.num_hours();
        let extra_minutes = diff.num_minutes() % 60;
        println!("{};{} worked today", hours, extra_minutes);
    }

    match tl.save() {
        Err(x) => {
            println!("Failed to save to logfile: {}", x);
            return 1;
        },
        Ok(_) => (),
    }

    return 0;
}

fn main() {
    let ret = real_main();
    std::process::exit(ret);
}
