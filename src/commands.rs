use chrono::{Datelike as _, Duration, Local, NaiveDate, NaiveTime, ParseResult};

use crate::timelog::TimeLogEntryType;
use crate::timelogger::TimeLogger;

pub fn parse_time_arg(s: &str) -> ParseResult<NaiveTime> {
    // %R = %H:%M
    // %H: hour, two digits
    // %M: minute, two digits
    NaiveTime::parse_from_str(s, "%R")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H.%M"))
        .or_else(|_| NaiveTime::parse_from_str(s, "%H"))
}

// TODO: Should this be utility function for map_err to customize error message?
pub fn get_time(s: Option<&str>) -> Result<NaiveTime, cli::Error> {
    match s {
        Some(x) => parse_time_arg(&x)
            .map_err(|e| cli::Error::App(format!("Failed to parse time from string {x}: {e}"))),
        None => Ok(Local::now().time()),
    }
}

fn save_log(timelogger: &mut TimeLogger) -> cli::Result {
    timelogger
        .save()
        .map_err(|e| cli::Error::App(format!("Failed to save to logfile: {e}")))
}

pub fn start(ctx: &mut cli::Globals, time: Option<String>) -> cli::Result {
    let time = get_time(time.as_deref())?;

    let tl: &mut TimeLogger = ctx.get::<TimeLogger>().expect("No global timelogger");
    let entry = tl.log_start(Local::today().naive_local(), time);

    save_log(tl)?;
    println!(
        "Logged: Starting {} at {}",
        entry.ty(),
        entry.start().expect("The start value was just set")
    );

    Ok(())
}

pub fn end(ctx: &mut cli::Globals, time: Option<String>) -> cli::Result {
    let time = get_time(time.as_deref())?;

    let tl: &mut TimeLogger = ctx.get::<TimeLogger>().expect("No global timelogger");
    let entry = tl.log_end(Local::today().naive_local(), time);

    if let Err(e) = tl.save() {
        return Err(cli::Error::App(format!("Failed to save to logfile: {e}")));
    }

    println!(
        "Logged: Ending {} at {}",
        entry.ty(),
        entry.end().expect("The end value was just set")
    );

    Ok(())
}

// TODO: Remove
struct Args {
    flag_with: Option<String>,
    flag_last: bool,
    flag_mon: bool,
    flag_tue: bool,
    flag_wed: bool,
    flag_thu: bool,
    flag_fri: bool,
}

fn fmt_dur(dur: Duration) -> String {
    format!("{};{}", dur.num_hours(), dur.num_minutes() % 60)
}

fn get_date_for_day_cmd(args: &Args) -> NaiveDate {
    use chrono::Weekday;

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

    date
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
    ret.to_string()
}

fn get_text_for_monthweek_cmd(args: &Args) -> String {
    match args.flag_last {
        true => "last",
        false => "this",
    }
    .to_string()
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

    date
}

fn get_date_for_month_cmd(args: &Args) -> NaiveDate {
    let today = Local::today().naive_local();
    match args.flag_last {
        false => today,
        true => NaiveDate::from_ymd(today.year(), today.month(), 1).pred(),
    }
}

pub fn month(ctx: &mut cli::Globals) -> cli::Result {
    let tl: &mut TimeLogger = ctx.get::<TimeLogger>().expect("No global timelogger");
    let args = todo!();

    let date = get_date_for_month_cmd(&args);
    let time = get_time(args.flag_with.as_deref())?;

    let flag_last: bool = todo!();

    let this_month = !flag_last;
    if !this_month {
        if let Some(bad_entries) = tl.verify_entries_in_month_of(date) {
            for e in bad_entries {
                println!("Incomplete entry: {}", e);
            }
        }
    }

    let time_opt = match this_month {
        true => Some(time),
        false => None,
    };

    let (time_left, _) = match tl.time_left_in_month_of_with(date, time_opt) {
        Ok(x) => x,
        Err(e) => {
            println!("Couldn't calculate time left this week: {}", e);
            return todo!();
        }
    };
    let time_worked = match tl.time_logged_in_month_of_with(date, time_opt) {
        Ok(x) => x,
        Err(e) => {
            println!("Couldn't calculate time worked this week: {}", e);
            return todo!();
        }
    };

    println!(
        "{} worked this month\n{} left this month",
        fmt_dur(time_worked),
        fmt_dur(time_left)
    );

    Ok(())
}

pub fn week(ctx: &mut cli::Globals) -> cli::Result {
    let tl: &mut TimeLogger = ctx.get::<TimeLogger>().expect("No global timelogger");
    let args = todo!();
    let date = get_date_for_week_cmd(&args);
    let week_text_fmt = get_text_for_monthweek_cmd(&args);
    let time = get_time(args.flag_with.as_deref())?;

    let this_week = !args.flag_last;
    if !this_week {
        if let Some(bad_entries) = tl.verify_entries_in_week_of(date) {
            for e in bad_entries {
                println!("Incomplete entry: {}", e);
            }
        }
    }

    let time_opt = match this_week {
        true => Some(time),
        false => None,
    };

    let (time_left, flex) = match tl.time_left_in_week_of_with(date, time_opt) {
        Ok(x) => x,
        Err(e) => {
            println!("Couldn't calculate time left {} week: {}", week_text_fmt, e);
            return todo!();
        }
    };
    let time_worked = match tl.time_logged_in_week_of_with(date, time_opt) {
        Ok(x) => x,
        Err(e) => {
            println!(
                "Couldn't calculate time worked {} week: {}",
                week_text_fmt, e
            );
            return todo!();
        }
    };

    println!(
        "{0} worked {3} week\n{1} left {3} week ({2} of which is flex)",
        fmt_dur(time_worked),
        fmt_dur(time_left),
        fmt_dur(flex),
        week_text_fmt
    );

    Ok(())
}

pub fn day(ctx: &mut cli::Globals) -> cli::Result {
    let tl: &mut TimeLogger = ctx.get::<TimeLogger>().expect("No global timelogger");
    let args = todo!();
    let date = get_date_for_day_cmd(&args);
    let day_text_fmt = get_text_for_day_cmd(&args);
    let today = !(args.flag_last
        || args.flag_mon
        || args.flag_tue
        || args.flag_wed
        || args.flag_thu
        || args.flag_fri);

    let time = get_time(args.flag_with.as_deref())?;
    let time_opt = match today {
        true => Some(time),
        false => None,
    };

    let worked_time = match tl.time_logged_at_date_with(date, time_opt) {
        Ok(x) => x,
        Err(e) => {
            return Err(cli::Error::App(format!(
                "Couldn't calculate time worked {}: {}",
                day_text_fmt, e
            )));
        }
    };
    println!("{} worked {}", fmt_dur(worked_time), day_text_fmt);
    Ok(())
}

pub fn view(ctx: &mut cli::Globals, n_entries: Option<usize>) -> cli::Result {
    let tl: &mut TimeLogger = ctx.get::<TimeLogger>().expect("No global timelogger");
    let n_entries = n_entries.unwrap_or(2);
    for tld in tl.get_latest_n_entries(n_entries) {
        println!("{}", tld);
    }

    Ok(())
}

pub fn batch(
    ctx: &mut cli::Globals,
    arg_type: String,
    from: String,
    to: String,
    weekday_only: Option<bool>,
) -> cli::Result {
    use std::str::FromStr;
    let tl: &mut TimeLogger = ctx.get::<TimeLogger>().expect("No global timelogger");

    let ty = match TimeLogEntryType::from_str(arg_type.as_str()) {
        Ok(x) => x,
        Err(e) => {
            println!("Failed to parse TimeLogEntryType for --type: {}", e);
            return Err(cli::Error::ArgParse(cli::ArgParseError));
        }
    };

    let naive_date_str = "%Y/%m/%d";

    let from = match NaiveDate::parse_from_str(from.as_str(), naive_date_str) {
        Ok(x) => x,
        Err(e) => {
            println!("Failed to parse NaiveDate for --from: {}", e);
            return Err(cli::Error::ArgParse(cli::ArgParseError));
        }
    };

    let to = match NaiveDate::parse_from_str(to.as_str(), naive_date_str) {
        Ok(x) => x,
        Err(e) => {
            println!("Failed to parse NaiveDate for --to: {}", e);
            return Err(cli::Error::ArgParse(cli::ArgParseError));
        }
    };

    if let Err(e) = tl.batch_add(ty, from, to, weekday_only.unwrap_or(false)) {
        return Err(cli::Error::App(format!("Batch command failed: {e}")));
    }

    save_log(tl)?;
    Ok(())
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
