extern crate chrono;

use std::path::PathBuf;
use std::path::Path;
use std::io::BufReader;
use std::io::prelude::*;
use std::fs::File;
use std;
use std::fs;
use std::fmt;
use std::io;
use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;
use std::str::FromStr;

use chrono::NaiveTime;
use chrono::Duration;
use chrono::Weekday;
use chrono::prelude::*;

use TimeLogError::{ParseError, TimeError, InvalidInputError, IOError};

pub type TimeLogResult<T> = std::result::Result<T, TimeLogError>;

#[derive(Debug)]
pub enum TimeLogError {
    ParseError(String),
    TimeError(chrono::ParseError),
    InvalidInputError(String),
    IOError(io::Error),
}

impl PartialEq for TimeLogError {
    fn eq(&self, other: &TimeLogError) -> bool {
        match (self, other) {
            (&ParseError(_), &ParseError(_)) => true,
            (&TimeError(_), &TimeError(_)) => true,
            (&InvalidInputError(_), &InvalidInputError(_)) => true,
            (&IOError(_), &IOError(_)) => true,
            _ => false,
        }
    }
}

impl TimeLogError {
	fn parse_error(s: &str) -> TimeLogError {
        TimeLogError::ParseError(String::from(s))
    }

    fn io_error_extra_msg(e: &io::Error, msg: &str) -> TimeLogError {
        TimeLogError::IOError(std::io::Error::new(e.kind(), msg))
    }

    fn other_io(msg: &str) -> TimeLogError {
        TimeLogError::IOError(std::io::Error::new(io::ErrorKind::Other, msg))
    }

}

impl Display for TimeLogError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TimeLogError::ParseError(ref s) => write!(f, "Parse error: {}", s),
            TimeLogError::IOError(ref err) => write!(f, "IO error: {}", err),
            TimeLogError::TimeError(ref err) => write!(f, "Time error: {}", err),
            TimeLogError::InvalidInputError(ref s) => write!(f, "Invalid input: {}", s),
        }
    }
}

impl Error for TimeLogError {
    fn description(&self) -> &str {
        match *self {
            TimeLogError::ParseError(ref s) => s.as_str(),
            TimeLogError::IOError(ref err) => err.description(),
            TimeLogError::TimeError(ref err) => err.description(),
            TimeLogError::InvalidInputError(ref s) => s.as_str(),
        }
    }
}

impl From<chrono::ParseError> for TimeLogError {
    fn from(err: chrono::ParseError) -> TimeLogError {
        TimeLogError::TimeError(err)
    }
}

impl From<io::Error> for TimeLogError {
    fn from(err: io::Error) -> TimeLogError {
        TimeLogError::IOError(err)
    }
}

impl From<std::num::ParseIntError> for TimeLogError {
    fn from(err: std::num::ParseIntError) -> TimeLogError {
        TimeLogError::ParseError(format!("{}", err))
    }
}

pub fn parse_duration(string: &str) -> TimeLogResult<Duration> {
    let s = string.trim();
    let m: i64;
    let h: i64;
    if s.contains(";") {
        let dur: Vec<&str> = s.split(';').map(|x| x.trim()).collect();
        h = dur[0].parse().unwrap_or(0);
        m = dur[1].parse().unwrap_or(0);
    } else {
        m = s.parse()?;
        h = 0;
    }

    return Ok(Duration::minutes(h * 60 + m));
}

const MAX_DAYS_IN_MONTH: usize = 31;
const MONTHS_IN_YEAR: usize = 12;
const MONTH_2_NDAYS: [usize; MONTHS_IN_YEAR] = [31,28,31,30,31,30,31,31,30,31,30,31];

/*
 * Format:
 * <NaiveDate>
 *   Start: <NaiveTime>
 *   End: <NaiveTime>
 *   Accumulated break: <Duration>
 * <NaiveDate>
 *   Start: <NaiveTime>
 *   End: <NaiveTime>
 *   Accumulated break: <Duration>
 */

#[derive(Copy, Clone, Debug)]
struct TimeLogDay {
    start: Option<NaiveTime>,
    end: Option<NaiveTime>,
    acc_break: Duration,
    date: NaiveDate,
}

impl TimeLogDay {
    fn new(date: NaiveDate) -> Self {
        TimeLogDay{start: None, end: None, acc_break: Duration::seconds(0), date: date}
    }

    fn set_start(&mut self, time: NaiveTime) {
        debug_assert!(time.nanosecond() == 0);
        self.start = Some(time);
    }

    fn set_end(&mut self, time: NaiveTime) {
        debug_assert!(time.nanosecond() == 0);
        self.end = Some(time);
    }

    fn add_break(&mut self, dur: Duration) {
        self.acc_break = self.acc_break + dur;
    }

    fn is_workday(&self) -> bool {
        self.date.weekday() != Weekday::Sat && self.date.weekday() != Weekday::Sun
    }
}

fn try_get_naivetime(s: &str) -> Option<NaiveTime> {
    if s.contains("UNDEF") {
        return None;
    } else {
        return NaiveTime::from_str(s).ok();
    }
}

macro_rules! TIMELOGDAY_NAIVEDATE_FORMAT_STRING {
    () => ("%d/%m/%Y %A");
}
macro_rules! start_format_str {
    ( $x:expr ) => (format!("  Start: {}\n", $x).as_str());
}
macro_rules! end_format_str {
    ( $x:expr ) => (format!("  End: {}\n", $x).as_str());
}

/*
 * <NaiveDate-format-string>
 *   Start: <NaiveTime>
 *   End: <NaiveTime>
 *   Accumulated break: <Duration>
 */

impl FromStr for TimeLogDay {
    type Err = TimeLogError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lines: Vec<&str> = s.lines().collect();
        if lines.len() != 4 {
            panic!("Not implemented");
        }

        let date = NaiveDate::parse_from_str(lines[0].trim(), TIMELOGDAY_NAIVEDATE_FORMAT_STRING!())?;
        let start = try_get_naivetime(lines[1].split(' ')
                                      .nth(1)
                                      .ok_or(TimeLogError::parse_error("Invalid format, can't parse start time"))?
                                      .trim());
        let end = try_get_naivetime(lines[2].split(' ')
                                    .nth(1)
                                    .ok_or(TimeLogError::parse_error("Invalid format, can't parse end time"))?
                                    .trim());
        let acc_br = parse_duration(lines[3].split(' ')
                                    .nth(2)
                                    .ok_or(TimeLogError::parse_error("Invalid format, can't parse accumulated break"))?
                                    .trim())?;
        return Ok(TimeLogDay{start: start, end:end, acc_break: acc_br, date: date});
    }
}

impl Display for TimeLogDay {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut s = String::new();
        s.push_str(self.date.format(TIMELOGDAY_NAIVEDATE_FORMAT_STRING!()).to_string().as_str());
        s.push('\n');
        if let Some(x) = self.start {
            s.push_str(start_format_str!(x));
        } else {
            s.push_str(start_format_str!("UNDEF"));
        }
        if let Some(x) = self.end {
            s.push_str(end_format_str!(x));
        } else {
            s.push_str(end_format_str!("UNDEF"));
        }
        s.push_str(&format!("  Accumulated break: {:02};{:02}\n", self.acc_break.num_hours(), self.acc_break.num_minutes() % 60));
        write!(f, "{}", s)
    }
}

struct TimeLogMonth {
    n_days: usize,
    days: Vec<TimeLogDay>,
}

fn get_first_day_in_week_of(date: NaiveDate) -> NaiveDate {
    let mut first_day = date;
    while first_day.weekday() != Weekday::Mon && first_day.day0() != 0 {
        first_day = first_day.pred();
    }

    return first_day;
}

fn get_last_day_in_week_of(date: NaiveDate) -> NaiveDate {
    let mut last_day = date;
    while last_day.weekday() != Weekday::Fri && last_day.day0() as usize != MONTH_2_NDAYS[date.month0() as usize] {
        last_day = last_day.succ();
    }

    return last_day;
}

impl TimeLogMonth {

    fn empty(first_date: NaiveDate) -> Self {
        let month = first_date.month0();
        let n_days = MONTH_2_NDAYS[month as usize];
        debug_assert!(n_days <= MAX_DAYS_IN_MONTH, "Number of days in month is too large");
        let mut days = Vec::with_capacity(n_days);
        for i in 0..n_days {
            days.push(TimeLogDay::new(NaiveDate::from_ymd(first_date.year(), first_date.month(), (i + 1) as u32)));
        }
        TimeLogMonth{n_days: n_days, days: days}
    }

    fn compute_time_worked_between(&self, day1_idx: usize, day2_idx: usize) -> Duration {
        self.days[day1_idx..day2_idx].iter().fold(Duration::zero(), |acc, day| {
            match (day.start, day.end) {
                (Some(start), Some(end)) => {
                    debug_assert!(end > start, "End of workday has to be after start");
                    return acc + end.signed_duration_since(start) - day.acc_break;
                },
                (_, _) => return acc,
            };
        })
    }

    fn compute_workable_time_between(&self, first_day_idx: usize, last_day_idx: usize) -> Duration {
        let n_work_days = self.days[first_day_idx..last_day_idx]
            .iter()
            .filter(|x| x.is_workday())
            .count();
        Duration::hours(n_work_days as i64 * 8)
    }

    fn compute_time_worked(&self) -> Duration {
        self.compute_time_worked_between(0, self.n_days)
    }

    fn compute_workable_time(&self) -> Duration {
        self.compute_workable_time_between(0, self.n_days)
    }

    fn compute_workable_time_in_week_of(&self, date: NaiveDate) -> Duration {
        let first_day_idx = get_first_day_in_week_of(date).day0() as usize;
        let last_day_idx = get_last_day_in_week_of(date).day0() as usize;
        self.compute_workable_time_between(first_day_idx, last_day_idx + 1)
    }

    fn compute_logged_time_in_week_of(&self, date: NaiveDate) -> Duration {
        let first_day_idx = get_first_day_in_week_of(date).day0() as usize;
        let last_day_idx = get_last_day_in_week_of(date).day0() as usize;
        self.compute_time_worked_between(first_day_idx, last_day_idx + 1)
    }

    fn compute_time_left_in_week_of(&self, date: NaiveDate) -> Duration {
        self.compute_workable_time_in_week_of(date) - self.compute_logged_time_in_week_of(date)
    }

    fn compute_time_left(&self) -> Duration {
        self.compute_workable_time() - self.compute_time_worked()
    }
}

impl FromStr for TimeLogMonth {
    type Err = TimeLogError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut days: Vec<TimeLogDay> = Vec::with_capacity(MAX_DAYS_IN_MONTH);
        let line_it = s.lines();

        let days_it = line_it.enumerate().fold(Vec::new(), |mut acc: Vec<String>, (i, x)| {
            if i % 4 == 0 {
                acc.push(String::new());
            }
            acc[i / 4].push_str(x.trim());
            acc[i / 4].push('\n');
            return acc;
        });

        for day in days_it {
            days.push(TimeLogDay::from_str(day.as_str())?);
        }

        Ok(TimeLogMonth{n_days: MONTH_2_NDAYS[days[0].date.month0() as usize], days: days})
    }
}

impl fmt::Display for TimeLogMonth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::new();
        for i in 0..self.n_days {
            s.push_str(self.days[i].to_string().as_str());
        }
        write!(f, "{}", s)
    }
}

pub struct TimeLogger {
    tl_month: TimeLogMonth,
    file_path: PathBuf,
}

const TIMELOGGER_FOLDER: &str = ".timelog";
impl TimeLogger {

    pub fn current_month() -> TimeLogResult<Self> {
        /* Directory structure is:
         * $HOME/.timelog/
         *   2017/
         *      January.tl
         *      February.tl
         *   2018/
         */
        let today = Local::today();
        let mut path_buf = std::env::home_dir().ok_or(TimeLogError::other_io("Can't find home dir"))?;

        // .timelog
        path_buf.push(TIMELOGGER_FOLDER);
        if !path_buf.as_path().exists() {
            std::fs::create_dir(path_buf.as_path())?;
        }

        // .timelog/year/
        path_buf.push(today.format("%Y/").to_string().as_str());
        if !path_buf.as_path().exists() {
            std::fs::create_dir(path_buf.as_path())?;
        }

        // .timelog/year/month.tl
        path_buf.push(today.format("%B").to_string().as_str());
        path_buf.set_extension("tl");

        if !path_buf.as_path().exists() {
            File::create(path_buf.as_path())?;
            return Ok(TimeLogger{tl_month: TimeLogMonth::empty(NaiveDate::from_ymd(today.year(), today.month(), 1)),
                                 file_path: path_buf});
        }

        let file = File::open(path_buf.as_path())?;

        let mut buf_reader = BufReader::new(file);
        let mut contents = String::new();
        buf_reader.read_to_string(&mut contents)?;
        let tlm: TimeLogMonth = match contents.parse() {
          Ok(x) => Ok(x),
          Err(e) => Err(io::Error::new(io::ErrorKind::InvalidData, format!("Unable to parse logfile: {}", e).as_str())),
        }?;

        Ok(TimeLogger{tl_month: tlm, file_path: path_buf})
    }

    fn time_worked_today_with(&self, end: NaiveTime) -> TimeLogResult<Duration> {
        let start = match self.todays_start() {
            Some(x) => x,
            None => return Err(InvalidInputError(String::from("No start value set today"))),
        };
        Ok(end.signed_duration_since(start) - self.todays_break())
    }

    pub fn time_worked_today(&self) -> TimeLogResult<Duration> {
        let end = self.todays_end().unwrap_or(Local::now().time());
        self.time_worked_today_with(end)
    }

    pub fn time_left_this_week(&self) -> Duration {
        let now = Local::now();
        self.tl_month.compute_time_left_in_week_of(now.naive_local().date())
            - match self.todays_end() {
                Some(_) => Duration::seconds(0), // We have already added this
                None => self.time_worked_today_with(Local::now().time()).unwrap_or(Duration::seconds(0)),
            }
    }

    pub fn hours_left_this_month(&self) -> u32 {
        self.tl_month.compute_time_left().num_hours() as u32
    }

    pub fn total_hours_this_month(&self) -> u32 {
        self.tl_month.compute_workable_time().num_hours() as u32
    }

    pub fn log_start(&mut self, time: NaiveTime) {
        let hms_time = NaiveTime::from_hms(time.hour(), time.minute(), time.second());
        self.tl_month.days[Local::now().day0() as usize].set_start(hms_time);
    }

    pub fn log_end(&mut self, time: NaiveTime) {
        let hms_time = NaiveTime::from_hms(time.hour(), time.minute(), time.second());
        self.tl_month.days[Local::now().day0() as usize].set_end(hms_time);
    }

    pub fn log_break(&mut self, dur: Duration) {
        self.tl_month.days[Local::now().day0() as usize].add_break(dur);
    }

    fn todays_start(&self) -> Option<NaiveTime> {
        self.tl_month.days[Local::today().day0() as usize].start
    }

    fn todays_end(&self) -> Option<NaiveTime> {
        self.tl_month.days[Local::today().day0() as usize].end
    }

    fn todays_break(&self) -> Duration {
        self.tl_month.days[Local::today().day0() as usize].acc_break
    }

    pub fn save(&self) -> TimeLogResult<()> {
        let mut bkp = self.file_path.clone();
        bkp.set_extension("tl.bkp");
        let bkp_fp = bkp.as_path();
        let fp: &Path = self.file_path.as_path();
        debug_assert!(fp.exists(), "logfile does not exist");
        let mut file = File::create(fp)?;
        let s = format!("{}", self.tl_month);
        fs::copy(fp, bkp_fp)?;
        match file.write_all(s.as_str().as_bytes()) {
            Ok(_) => fs::remove_file(bkp_fp)?,
            Err(ref e) => {
                fs::copy(bkp_fp, fp)?;
                return Err(TimeLogError::io_error_extra_msg(e, format!("Failed to write to file (restoring backup): {}", e).as_str()));
            },
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
use super::*;
use chrono::NaiveTime;
use chrono::Duration;
    #[test]
    fn timelogday_basic_mutators() {
        let mut day = TimeLogDay::new(NaiveDate::from_ymd(2016, 1, 1));
        let start_time = NaiveTime::from_hms(11, 30, 0);
        day.set_start(start_time);
        assert_eq!(day.start, Some(start_time));
        let end_time = NaiveTime::from_hms(12, 30, 0);
        day.set_end(end_time);
        assert_eq!(day.end, Some(end_time));
        let dur1 = Duration::seconds(60);;
        let dur2 = Duration::minutes(31);;
        day.add_break(dur1);
        day.add_break(dur2);
        assert_eq!(day.acc_break, Duration::minutes(32));
        assert_eq!(day.acc_break, Duration::seconds(32 * 60));
    }

    #[test]
    fn timelogday_is_workday() {
        let mon = TimeLogDay::new(NaiveDate::from_ymd(2017, 11, 20));
        let tue = TimeLogDay::new(NaiveDate::from_ymd(2017, 11, 21));
        let wed = TimeLogDay::new(NaiveDate::from_ymd(2017, 11, 22));
        let thu = TimeLogDay::new(NaiveDate::from_ymd(2017, 11, 23));
        let fri = TimeLogDay::new(NaiveDate::from_ymd(2017, 11, 24));
        let sat = TimeLogDay::new(NaiveDate::from_ymd(2017, 11, 25));
        let sun = TimeLogDay::new(NaiveDate::from_ymd(2017, 11, 26));
        assert!(mon.is_workday());
        assert!(tue.is_workday());
        assert!(wed.is_workday());
        assert!(thu.is_workday());
        assert!(fri.is_workday());
        assert!(!sun.is_workday());
        assert!(!sat.is_workday());
    }

    #[test]
    fn timelogday_from_str() {
        let all_undef = "01/11/2017 Wednesday\nStart: UNDEF\nEnd: UNDEF\nAccumulated break: 00;00";

        let day: TimeLogDay = all_undef.parse().unwrap();
        assert_eq!(day.start, None);
        assert_eq!(day.end, None);
        assert_eq!(day.acc_break, Duration::seconds(0));
        assert_eq!(day.date.day(), 1);
        assert_eq!(day.date.month(), 11);
        assert_eq!(day.date.year(), 2017);
        assert_eq!(day.date.weekday(), Weekday::Wed);

        let start = "02/11/2017 Thursday\nStart: 07:31:00\nEnd: UNDEF\nAccumulated break: 00;00";
        let start_day: TimeLogDay = start.parse().unwrap();
        assert_eq!(start_day.start, Some(NaiveTime::from_hms(7, 31, 0)));
        assert_eq!(start_day.end, None);
        assert_eq!(start_day.acc_break, Duration::seconds(0));
        assert_eq!(start_day.date.day(), 2);
        assert_eq!(start_day.date.month(), 11);
        assert_eq!(start_day.date.year(), 2017);
        assert_eq!(start_day.date.weekday(), Weekday::Thu);

        let start_end = "02/11/2017 Thursday\nStart: 07:31:00\nEnd: 12:00:00\nAccumulated break: 00;00";
        let start_end_day: TimeLogDay = start_end.parse().unwrap();
        assert_eq!(start_end_day.start, Some(NaiveTime::from_hms(7, 31, 0)));
        assert_eq!(start_end_day.end, Some(NaiveTime::from_hms(12, 0, 0)));
        assert_eq!(start_end_day.acc_break, Duration::seconds(0));

        let s_e_break = "02/11/2017 Thursday\nStart: 07:31:00\nEnd: 12:00:00\nAccumulated break: 00;35";
        let mut start_end_day: TimeLogDay = s_e_break.parse().unwrap();
        assert_eq!(start_end_day.start, Some(NaiveTime::from_hms(7, 31, 0)));
        assert_eq!(start_end_day.end, Some(NaiveTime::from_hms(12, 0, 0)));
        assert_eq!(start_end_day.acc_break, Duration::seconds(35 * 60));

        start_end_day.add_break(Duration::minutes(21));
        assert_eq!(start_end_day.acc_break, Duration::minutes(35 + 21));

        start_end_day.add_break(Duration::minutes(13));
        assert_eq!(start_end_day.acc_break, Duration::minutes(35 + 21 + 13));
    }

    #[test]
    fn parse_duration_input() {
        assert_eq!(super::parse_duration("00;30"), Ok(Duration::minutes(30)));
        assert_eq!(super::parse_duration("0;30"), Ok(Duration::minutes(30)));
        assert_eq!(super::parse_duration("0;3"), Ok(Duration::minutes(3)));
        assert_eq!(super::parse_duration("1;3"), Ok(Duration::minutes(60 + 3)));
        assert_eq!(super::parse_duration("1;03"), Ok(Duration::minutes(60 + 3)));

        assert_eq!(super::parse_duration("30"), Ok(Duration::minutes(30)));
        assert_eq!(super::parse_duration("120"), Ok(Duration::minutes(120)));

        assert_eq!(super::parse_duration(";30"), Ok(Duration::minutes(30)));
        assert_eq!(super::parse_duration(";120"), Ok(Duration::minutes(120)));
    }

    #[test]
    fn timelogmonth_empty() {
        let tlm = TimeLogMonth::empty(NaiveDate::from_ymd(2017, 05, 1));
        assert_eq!(tlm.compute_time_worked(), Duration::seconds(0));
        assert_eq!(tlm.compute_time_left(), tlm.compute_workable_time());
        assert_eq!(tlm.compute_time_worked_between(0, 4), Duration::seconds(0));
        assert_eq!(tlm.compute_workable_time_between(0, 0), Duration::hours(0));
        for i in 0..6 {
            assert_eq!(tlm.compute_workable_time_between(0,i), Duration::hours(8 * i as i64));
        }
    }

    #[test]
    fn timelogmonth_basic() {
        let may_1st = NaiveDate::from_ymd(2017, 05, 01);
        let may_8th = NaiveDate::from_ymd(2017, 05, 08);
        let mut tlm = TimeLogMonth::empty(may_1st);
        tlm.days[0].set_start(NaiveTime::from_hms(8,15,0));
        assert_eq!(tlm.compute_time_worked(), Duration::seconds(0));
        tlm.days[0].set_end(NaiveTime::from_hms(10,15,0));
        assert_eq!(tlm.compute_time_worked(), Duration::hours(2));

        tlm.days[1].set_start(NaiveTime::from_hms(8,15,0));
        assert_eq!(tlm.compute_time_worked(), Duration::hours(2));
        tlm.days[1].set_end(NaiveTime::from_hms(10,15,0));
        assert_eq!(tlm.compute_time_worked(), Duration::hours(4));

        tlm.days[7].set_start(NaiveTime::from_hms(7,15,0));
        assert_eq!(tlm.compute_time_worked(), Duration::hours(4));
        tlm.days[7].set_end(NaiveTime::from_hms(19,0,0));
        assert_eq!(tlm.compute_time_worked(), Duration::minutes(15*60 + 45));

        tlm.days[8].add_break(Duration::minutes(19));
        assert_eq!(tlm.compute_time_worked(), Duration::minutes(15*60 + 45));

        tlm.days[11].add_break(Duration::minutes(35));
        tlm.days[11].set_start(NaiveTime::from_hms(8,0,0));
        tlm.days[11].set_end(NaiveTime::from_hms(16,0,0));
        assert_eq!(tlm.compute_time_worked(), Duration::minutes(15*60 + 45 + 8 * 60 - 35));

        assert_eq!(tlm.compute_workable_time_in_week_of(may_1st), Duration::hours(40));
        assert_eq!(tlm.compute_logged_time_in_week_of(may_1st), Duration::hours(4));
        assert_eq!(tlm.compute_logged_time_in_week_of(may_8th), Duration::minutes(11 * 60 + 45 - 35 + 8 * 60));
    }
}
