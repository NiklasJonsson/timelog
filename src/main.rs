#[macro_use]
extern crate serde_derive;
extern crate docopt;
extern crate chrono;

mod timelog;
mod timelogger;

use docopt::Docopt;
use chrono::prelude::*;
use chrono::NaiveTime;
use chrono::Duration;
use chrono::ParseResult;
use timelogger::TimeLogger;
use timelog::TimeLogError;

const USAGE: &'static str = "
Timelog - log time

Usage:
  timelog start [<time>]
  timelog end [<time>]
  timelog month [--with <time>]
  timelog week [--with <time>]
  timelog week [--last]
  timelog day [--with <time>]
  timelog day [--last]
  timelog day [--mon | --tue | --wed | --thu | --fri]
  timelog view <n-entries>
  timelog (-h | --help)

Options:
  -h, --help                Show this screen.
  -w, --with <time>         If there is no end time for an entry, this will be used instead.
";

#[derive(Debug, Deserialize)]
struct Args {
    cmd_start: bool,
    cmd_end: bool,
    cmd_month: bool,
    cmd_week: bool,
    cmd_day: bool,
    cmd_view: bool,
    arg_time: Option<String>,
    arg_n_entries: Option<usize>,
    flag_with: Option<String>,
    flag_last: bool,
    flag_mon: bool,
    flag_tue: bool,
    flag_wed: bool,
    flag_thu: bool,
    flag_fri: bool,
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
    } else if args.flag_fri {
        target = Weekday::Fri;
    } else if args.flag_last {
        target = date.pred().weekday();
    };

    while date.weekday() != target {
        date = date.pred();
    }

    return date;
}

fn get_text_for_day_cmd(args: &Args) -> String {
    let ret: &str;
    if args.flag_mon {
        ret = "last monday";
    } else if args.flag_tue {
        ret = "last tuesday";
    } else if args.flag_wed {
        ret = "last wednesday";
    } else if args.flag_thu {
        ret = "last thursday";
    } else if args.flag_fri {
        ret = "last friday";
    } else if args.flag_last {
        ret = "yesterday"
    } else {
        ret = "today";
    }
    return ret.to_string();
}

fn get_text_for_monthweek_cmd(args: &Args) -> String {
    match args.flag_last {
        true => "last",
        false => "this"
    }.to_string()
}

fn get_date_for_week_cmd(args: &Args) -> NaiveDate {
    let mut date = Local::today().naive_local();
    let target = date.weekday();
    if args.flag_last {
        date = date.pred();
    }

    while date.weekday() != target {
        date = date.pred();
    }

    return date;
}

fn get_date_for_month_cmd(args: &Args) -> NaiveDate {
    let today = Local::today().naive_local();
    match args.flag_last {
        false => today,
        true => NaiveDate::from_ymd(today.year(), today.month(), 1).pred()
    }
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
        let date = get_date_for_month_cmd(&args);
        let time  = match get_time(args.flag_with) {
            Ok(t) => t,
            Err(e) => {
                println!("Unable to parse args: {}", e);
                return 1;
            },
        };

        let this_month = !args.flag_last;

        let time_opt = match this_month {
            true => Some(time),
            false => None,
        };

        let (time_left, _) = match tl.time_left_in_month_of_with(date, time_opt) {
            Ok(x) => x,
            Err(e) => {
                println!("Couldn't calculate time left this week: {}", e);
                return 1;
            },
        };
        let time_worked = match tl.time_logged_in_month_of_with(date, time_opt) {
            Ok(x) => x,
            Err(e) => {
                println!("Couldn't calculate time worked this week: {}", e);
                return 1;
            },
        };

        println!("{} worked this month\n{} left this month",
        fmt_dur(time_worked),
        fmt_dur(time_left));
    } else if args.cmd_week {
        let date = get_date_for_week_cmd(&args);
        let week_text_fmt = get_text_for_monthweek_cmd(&args);
        let time  = match get_time(args.flag_with) {
            Ok(t) => t,
            Err(e) => {
                println!("Unable to parse args: {}", e);
                return 1;
            },
        };

        let this_week = !args.flag_last;

        let time_opt = match this_week {
            true => Some(time),
            false => None,
        };

        let (time_left, flex) = match tl.time_left_in_week_of_with(date, time_opt) {
            Ok(x) => x,
            Err(e) => {
                println!("Couldn't calculate time left {} week: {}", week_text_fmt, e);
                return 1;
            },
        };
        let time_worked = match tl.time_logged_in_week_of_with(date, time_opt) {
            Ok(x) => x,
            Err(e) => {
                println!("Couldn't calculate time worked {} week: {}", week_text_fmt, e);
                return 1;
            },
        };

        println!("{0} worked {3} week\n{1} left {3} week ({2} of which is flex)",
        fmt_dur(time_worked),
        fmt_dur(time_left),
        fmt_dur(flex),
        week_text_fmt);
    } else if args.cmd_day {
        let date = get_date_for_day_cmd(&args);
        let day_text_fmt = get_text_for_day_cmd(&args);
        let today = !(args.flag_last || args.flag_mon || args.flag_tue || args.flag_wed || args.flag_thu || args.flag_fri);

        let time  = match get_time(args.flag_with) {
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

        let worked_time = match tl.time_logged_at_date_with(date, time_opt) {
            Ok(x) => x,
            Err(e) => {
                println!("Couldn't calculate time worked {}: {}", day_text_fmt, e);
                return 1;
            }
        };
        println!("{} worked {}", fmt_dur(worked_time), day_text_fmt);
    } else if args.cmd_view {
        debug_assert!(args.arg_n_entries.is_some());
        for tld in tl.get_latest_n_entries(args.arg_n_entries.unwrap()) {
            println!("{}", tld);
        }
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
