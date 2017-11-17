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

const USAGE: &'static str = "
Timelog

Usage:
  timelog start [<time>]
  timelog end [<time>]
  timelog break [<duration>]
  timelog auto
  timelog (month | week)
  timelog
  timelog (-h | --help)

Options:
  -h --help     Show this screen.
";

// TODO: Nice features:
// Fix panic! on error: print to stderr and exit with 1
// How many hours have I worked today?
// How many hours left today?
// How many hours have i Worked this week?
// How many hours left his week?
// Remaing hours / remaiing days (avg)
// Parse duration and time more leniently, e.g. "6:30" or ";35"
// Close to optimal schedule?
// Given some constraints, how should I plan my week?
// e.g. end 16.15 tuesday and thursday. Start 9.15 mon/wed.
// End at 15 friday. Wednesaday, end at 17.30

#[derive(Debug, Deserialize)]
struct Args {
	cmd_start: bool,
	cmd_end: bool,
	cmd_break: bool,
	cmd_auto: bool,
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

fn get_duration(s: Option<String>) -> Result<Duration, String> {
    match s {
        Some(x) => timelog::parse_duration(&x),
        None => Err(String::from("None value can't be parsed")),
    }
}

fn main() {
	let args: Args = Docopt::new(USAGE)
		.and_then(|d| d.deserialize())
		.unwrap_or_else(|e| e.exit());

    let mut tl = match TimeLogger::current_month() {
      Ok(x) => x,
      Err(e) => {
        panic!("ERROR: {}", e);
      }
    };

    if args.cmd_start {
        let time = match get_time(args.arg_time) {
            Ok(t) => t,
            Err(e) => panic!("ERROR: {:?}", e),
        };
        tl.log_start(time);
    } else if args.cmd_end {
        let time = match get_time(args.arg_time) {
            Ok(t) => t,
            Err(e) => panic!("ERROR: {:?}", e),
        };
        tl.log_end(time);
    } else if args.cmd_break {
        let dur = match get_duration(args.arg_duration) {
            Ok(d) => d,
            Err(e) => panic!("ERROR: {:?}", e),
        };
        tl.log_break(dur);
    } else if args.cmd_auto {
        debug_assert!(false, "Not implemented yet!");
    } else {
        // TODO: Add hpurs left this week
        println!("{} left of {} this month", tl.hours_left_this_month(), tl.total_hours_this_month());
    }
    match tl.save() {
        Err(x) => println!("Failed to save to logfile: {}", x),
        Ok(_) => (),
    }
}
