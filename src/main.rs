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
use timelog::TimeLogError;

const USAGE: &'static str = "
Timelog

Usage:
  timelog start [<time>]
  timelog end [<time>]
  timelog month
  timelog week
  timelog day [--with-end=<time>]
  timelog day [--last]
  timelog day [--mon | --tue | --wed | --thu]
  timelog
  timelog (-h | --help)

Options:
  -h --help     Show this screen.
";

#[derive(Debug, Deserialize)]
struct Args {
    cmd_start: bool,
    cmd_end: bool,
    cmd_month: bool,
    cmd_week: bool,
    cmd_day: bool,
    arg_time: Option<String>,
    arg_duration: Option<String>,
    flag_with_end: Option<String>,
    flag_last: bool,
    flag_mon: bool,
    flag_tue: bool,
    flag_wed: bool,
    flag_thu: bool,
}

fn parse_time_arg(s: &String) -> ParseResult<NaiveTime> {
    // %R = %H:%M
    // %H: hour, two digits
    // %M: minute, two digits
    NaiveTime::parse_from_str(s, "%R")
    .or(NaiveTime::parse_from_str(s, "%H.%M"))
    .or(NaiveTime::parse_from_str(s, "%H"))
}

fn get_time(s: Option<String>) -> ParseResult<NaiveTime> {
    match s {
        Some(x) => parse_time_arg(&x),
        None => Ok(Local::now().time()),
    }
}

fn fmt_dur(dur: Duration) -> String {
    format!("{};{}", dur.num_hours(), dur.num_minutes() % 60)
}

fn get_date_for_day_cmd(args: &Args) -> NaiveDate {
    let mut date = Local::today().naive_local();
    let mut target = date.weekday();
    if args.flag_mon {
        target = Weekday::Mon;
    } else if args.flag_tue {
        target = Weekday::Tue;
    } else if args.flag_wed {
        target = Weekday::Wed;
    } else if args.flag_thu {
        target = Weekday::Thu;
    } else if args.flag_last {
        target = date.pred().weekday();
    } else {
        debug_assert!(!args.flag_last
                      || !args.flag_mon
                      || !args.flag_tue
                      || !args.flag_wed
                      || !args.flag_thu);

    }
    while date.weekday() != target {
        date = date.pred();
    }

    return date;
}

fn real_main() -> i32 {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    let mut tl = match TimeLogger::default() {
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
        tl.log_start(Local::today().naive_local(), time);
    } else if args.cmd_end {
        let time = match get_time(args.arg_time) {
            Ok(t) => t,
            Err(e) => {
                println!("Unable to update timelog: {}", e);
                return 1;
            },
        };
        tl.log_end(Local::today().naive_local(), time);
    } else if args.cmd_month {
        println!("{} hrs left of {} this month", tl.hours_left_this_month(), tl.total_hours_this_month());
    } else if args.cmd_week {
        let (time_left, flex) = match tl.time_left_this_week() {
            Ok(x) => x,
            Err(e) => {
                println!("Couldn't calculate time left this week: {}", e);
                return 1;
            },
        };
        let time_worked = match tl.time_worked_this_week() {
            Ok(x) => x,
            Err(e) => {
                println!("Couldn't calculate time worked this week: {}", e);
                return 1;
            },
        };

        println!("{} worked this week\n{} left this week ({} of which is flex)",
        fmt_dur(time_worked),
        fmt_dur(time_left),
        fmt_dur(flex));
    } else if args.cmd_day {
        let date = get_date_for_day_cmd(&args);
        let today = !(args.flag_last || args.flag_mon || args.flag_tue || args.flag_wed || args.flag_thu);

        let worked_time: Duration;
        let time  = match get_time(args.flag_with_end) {
            Ok(t) => t,
            Err(e) => {
                println!("Unable to parse args: {}", e);
                return 1;
            },
        };

        let time_opt = match today {
            true => Some(time),
            false => None,
        };

        let worked_time = match tl.time_worked_at_date_with(date, time_opt) {
            Ok(x) => x,
            Err(e) => {
                println!("Couldn't calculate time worked today: {}", e);
                return 1;
            }
        };
        println!("{} worked today", fmt_dur(worked_time));
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

#[cfg(test)]
mod main_tests {
use chrono::NaiveTime;
use super::*;
    #[test]
    fn parse_time() {
        assert_eq!(parse_time_arg(&String::from("03:00")), Ok(NaiveTime::from_hms(3,0,0)));
        assert_eq!(parse_time_arg(&String::from("03.00")), Ok(NaiveTime::from_hms(3,0,0)));
        assert_eq!(parse_time_arg(&String::from("3.00")), Ok(NaiveTime::from_hms(3,0,0)));
        assert_eq!(parse_time_arg(&String::from("3.0")), Ok(NaiveTime::from_hms(3,0,0)));
        assert_eq!(parse_time_arg(&String::from("03.0")), Ok(NaiveTime::from_hms(3,0,0)));
        assert_eq!(parse_time_arg(&String::from("3.00")), Ok(NaiveTime::from_hms(3,0,0)));
    }
}
