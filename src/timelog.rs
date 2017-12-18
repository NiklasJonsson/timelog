extern crate chrono;

use std::path::PathBuf;
use std::path::Path;
use std::io::BufReader;
//use std::iter::Peekable;
//use std::str::Lines;
use std::io::prelude::*;
use std::fs::File;
use std;
use std::fs;
use std::cmp::Ordering;
use std::fmt;
use std::io;
use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;
use std::str::FromStr;
use std::collections::HashMap;
use std::collections::hash_map::Entry::{Occupied, Vacant};

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
    fn parse_error(s: String) -> TimeLogError {
        TimeLogError::ParseError(s)
    }
    fn inv_inp(s: &str) -> TimeLogError {
        TimeLogError::InvalidInputError(String::from(s))
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

const MONTH_2_NDAYS: [u32; 12] = [31,28,31,30,31,30,31,31,30,31,30,31];

/*
 * Format (date start end type):
 * <NaiveDate> <Duration> <Duration> <TimeLogEntryType>
 */

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
enum TimeLogEntryType {
    Work,
    Sickness,
    Vacation,
    ParentalLeave,
}

impl fmt::Display for TimeLogEntryType {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl FromStr for TimeLogEntryType {
    type Err = TimeLogError;

    fn from_str(s: &str) -> TimeLogResult<TimeLogEntryType> {
        match s {
            "Work" => Ok(TimeLogEntryType::Work),
            "ParentalLeave" => Ok(TimeLogEntryType::ParentalLeave),
            "Vacation" => Ok(TimeLogEntryType::Vacation),
            "Sickness" => Ok(TimeLogEntryType::Sickness),
            _ => return Err(TimeLogError::parse_error(format!("Can't parse: {} as TimeLogEntryType", s))),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct TimeLogEntry {
    start: Option<NaiveTime>,
    end: Option<NaiveTime>,
    entry_type: TimeLogEntryType,
    date: NaiveDate,
}

impl Ord for TimeLogEntry {
    fn cmp(&self, other: &TimeLogEntry) -> Ordering {
        match (self.start, self.end, self.entry_type, self.date,
               other.start, other.end, other.entry_type, other.date) {
            (_,Some(end),_,_,
             Some(start),_,_,_)   => end.cmp(&start),
            (Some(start),_,_,_,
             _,Some(end),_,_)     => start.cmp(&end),
            (Some(start0),_,_,_,
             Some(start1),_,_,_)    => start0.cmp(&start1),
            (_,Some(end0),_,_,
             _,Some(end1),_,_)    => end0.cmp(&end1),
            (_,_,et0,_,
             _,_,et1,_)            => et0.cmp(&et1),
        }
    }
}

impl PartialOrd for TimeLogEntry {
    fn partial_cmp(&self, other: &TimeLogEntry)  -> Option<Ordering> {
        Some(self.cmp(other))
    }
}


impl TimeLogEntry {
    fn new(date: NaiveDate, entry_type: TimeLogEntryType) -> Self {
        TimeLogEntry{date: date, entry_type: entry_type, start: None, end: None}
    }

    fn start(date: NaiveDate, entry_type: TimeLogEntryType, time: NaiveTime) -> Self {
        TimeLogEntry{date: date, entry_type: entry_type, start: Some(time), end: None}
    }

    fn end(date: NaiveDate, entry_type: TimeLogEntryType, time: NaiveTime) -> Self {
        TimeLogEntry{date: date, entry_type: entry_type, start: None, end: Some(time)}
    }

    fn set_start(&mut self, time: NaiveTime) {
        debug_assert!(time.nanosecond() == 0);
        self.start = Some(time);
    }

    fn set_end(&mut self, time: NaiveTime) {
        debug_assert!(time.nanosecond() == 0);
        self.end = Some(time);
    }

}

fn try_get_naivetime(s: &str) -> Option<NaiveTime> {
    if s.contains("UNDEF") {
        return None;
    } else {
        return NaiveTime::from_str(s).ok();
    }
}

macro_rules! TIMELOGENTRY_NAIVEDATE_FORMAT_STRING {
    () => ("%Y/%m/%d %a");
}

impl FromStr for TimeLogEntry {
    type Err = TimeLogError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut delim_split = s.split('|');

        let date: NaiveDate = NaiveDate::parse_from_str(delim_split
                                                        .next()
                                                        .ok_or_else(|| TimeLogError::parse_error(
                                                                format!("Can't read date from: {}", s)))?
                                                        .trim(),
                                                        TIMELOGENTRY_NAIVEDATE_FORMAT_STRING!())?;

        let mut space_split = delim_split
            .next()
            .ok_or_else(|| TimeLogError::parse_error(
                    format!("Invalid format for entry: {}", s)))?
            .trim()
            .split(' ');

        let entry_type: TimeLogEntryType = space_split
            .next()
            .ok_or_else(|| TimeLogError::parse_error(
                    format!("Can't read type from: {}", s)))?
            .trim()
            .parse()?;

        let start = try_get_naivetime(space_split
                                      .next()
                                      .ok_or_else(|| TimeLogError::parse_error(
                                              format!("Can't read start from: {}", s)))?
                                      .trim());
        let end = try_get_naivetime(space_split
                                    .next()
                                    .ok_or_else(|| TimeLogError::parse_error(
                                            format!("Can't read end from: {}", s)))?
                                    .trim());
        return Ok(TimeLogEntry{date: date, entry_type: entry_type, start: start, end:end});
    }
}

fn opt_naivetime_to_str(ont: Option<NaiveTime>) -> String {
    match ont {
        Some(x) => format!("{}", x),
        None => "UNDEF".into(),
    }
}

impl Display for TimeLogEntry {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{} | {} {} {}",
               self.date.format(TIMELOGENTRY_NAIVEDATE_FORMAT_STRING!()),
               self.entry_type,
               opt_naivetime_to_str(self.start), 
               opt_naivetime_to_str(self.end))
    }
}

#[derive(Debug, PartialEq)]
struct TimeLogDay {
    // TODO: Print warnings if UNDEF is present on days that have already passed
    date: NaiveDate,
    entries: Vec<TimeLogEntry>,
}

impl From<TimeLogEntry> for TimeLogDay {
    fn from(entry: TimeLogEntry) -> TimeLogDay {
        let mut v = Vec::new();
        v.push(entry);
        TimeLogDay{date: entry.date, entries: v}
    }
}

macro_rules! gen_set {
    ($func: ident, $entry_field: ident, $entry_mutator: ident, $ctor: path) => {
        fn $func(&mut self, time: NaiveTime, entry_type: TimeLogEntryType) {
            debug_assert!(time.nanosecond() == 0);
            let mut found = false;
            for mut entry in &mut self.entries {
                if entry.$entry_field.is_none() && entry.entry_type == entry_type {
                    entry.$entry_mutator(time);
                    found = true;
                    break;
                }
            }

            if !found {
                self.entries.push($ctor(self.date, entry_type, time));
            }
            self.entries.sort();
        }
    }
}

fn is_weekday(date: NaiveDate) -> bool {
    date.weekday() != Weekday::Sat && date.weekday() != Weekday::Sun
}

impl TimeLogDay {
    fn is_weekday(&self) -> bool {
        is_weekday(self.date)
    }

    #[cfg(debug_assertions)]
    fn validate_ordering(&self) -> bool {
        for i in 0..self.entries.len() {
            for j in (i + 1)..self.entries.len() {
                let s_i = self.entries[i].start;
                let s_j = self.entries[j].start;
                let e_i = self.entries[i].end;
                let e_j = self.entries[j].end;
                if s_i.is_some() && s_j.is_some() {
                    debug_assert!(self.entries[i].start.unwrap() < self.entries[j].start.unwrap());
                }
                if e_i.is_some() && e_j.is_some() {
                    debug_assert!(self.entries[i].end.unwrap() < self.entries[j].end.unwrap());
                }

                if e_i.is_some() && s_j.is_some() {
                    debug_assert!(self.entries[i].end.unwrap() <= self.entries[j].start.unwrap());
                }
            }
        }
        return true;
    }

    fn add_entry(&mut self, e: TimeLogEntry) {
        self.entries.push(e);
    }

    fn empty(date: NaiveDate) -> TimeLogDay {
        TimeLogDay{date: date, entries: Vec::new()}
    }

    // TODO: Remove TimeLogEntry::?
    gen_set!(set_end, end, set_end, TimeLogEntry::end);
    gen_set!(set_start, start, set_start, TimeLogEntry::start);

    // TODO: Macro for these
    fn get_start(&self, etype: TimeLogEntryType) -> Option<NaiveTime> {
        debug_assert!(self.validate_ordering());
        for e in &self.entries {
            if let Some(s) = e.start {
                if e.entry_type == etype {
                    return Some(s);
                }
            }
        }
        return None;
    }

    fn get_end(&self, etype: TimeLogEntryType) -> Option<NaiveTime> {
        debug_assert!(self.validate_ordering());
        for e in self.entries.iter().rev() {
            if let Some(s) = e.end {
                if e.entry_type == etype {
                    return Some(s);
                }
            }
        }
        return None;
    }

    // TODO: Test
    fn time_logged_with(&self, end: NaiveTime, etype: TimeLogEntryType) -> TimeLogResult<Duration> {
        let mut dur = self.logged_time(etype);

        if self.entries.len() == 0 {
            return Err(TimeLogError::inv_inp("No entries today"));
        } else if self.entries.iter().all(|e| e.start.is_none()) {
            return Err(TimeLogError::inv_inp("No start entries today"));
        }

        // If we find an an entry with Some, None then we use end to add extra time to dur
        for e in &self.entries {
            if e.start.is_some() && e.end.is_none() && e.entry_type == etype {
                dur = dur + end.signed_duration_since(e.start.unwrap());
                break;
            }
        }
        return Ok(dur);
    }

    /* TODO: Consume only as many lines as needed from the iterator
       fn from_iterator(pk: &mut Peekable<Lines>) -> TimeLogResult<TimeLogDay> {
       let mut entries = Vec::new();
       let date = NaiveDate::parse_from_str(
       pk.peek().ok_or_else(|| TimeLogError::inv_inp("Not enough lines to read TimeLogDay"))?,
       TIMELOGDAY_NAIVEDATE_FORMAT_STRING!())
       .or_else(|_| Err(TimeLogError::parse_error("Unable to parse date for TimeLogDay".into())))?;
       pk.next(); // Consume date line
       let mut count = 0;

       while let Some(line) = pk.take_while(|line| line.trim().parse<TimeLogEntry>().is_ok()) {
       if let Ok(entry) = line.trim().parse() {
       entries.push(entry);
       } else {
       break;
       }
       pk.next();
       }
       Ok(TimeLogDay{date: date, entries: entries})
       }
       */

    fn loggable_time(&self, etype: TimeLogEntryType) -> Duration {
        if is_weekday(self.date) {
            Duration::hours(8)
        } else {
            Duration::hours(0)
        }
    }

    fn logged_time(&self, etype: TimeLogEntryType) -> Duration {
        let mut sum = Duration::seconds(0);
        for e in &self.entries {
            debug_assert!(is_weekday(self.date));
            if let (Some(start), Some(end)) = (e.start, e.end) {
                debug_assert!(e.start < e.end);
                if e.entry_type == etype {
                    sum = sum + end.signed_duration_since(start);
                }
            }
        }

        return sum;
    }
}

impl Display for TimeLogDay {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut s: String = String::new();
        for (i, entry) in self.entries.iter().enumerate() {
            if i != self.entries.len() - 1 {
                s.push_str(format!("{}\n", entry).as_str());
            } else {
                s.push_str(format!("{}", entry).as_str());
            }

        }
        write!(f, "{}", s.as_str())
    }
}

impl FromStr for TimeLogDay {
    type Err = TimeLogError;

    fn from_str(s: &str) -> TimeLogResult<TimeLogDay> {
        let mut lines = s.lines();
        let mut v: Vec<TimeLogEntry> = Vec::new();
        while let Some(l) = lines.next() {
            v.push(l.trim().parse()?);
        }
        debug_assert!(v.len() != 0);
        debug_assert!(v.iter().all(|x| x.date == v[0].date));

        Ok(TimeLogDay{date: v[0].date, entries: v})
    }
}

fn get_first_day_in_week_of(date: NaiveDate) -> NaiveDate {
    let mut first_day = date;
    while first_day.weekday() != Weekday::Mon {
        first_day = first_day.pred();
    }

    return first_day;
}

fn get_last_day_in_week_of(date: NaiveDate) -> NaiveDate {
    // TODO: Cleanup?
    let mut friday = date;
    if date.weekday() == Weekday::Sat || date.weekday() == Weekday::Sun {
        while friday.weekday() != Weekday::Fri {
            friday = friday.pred();
        }
    } else {
        while friday.weekday() != Weekday::Fri {
            friday = friday.succ();
        }
    }

    return friday;
}

fn get_first_day_in_month_of(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd(date.year(), date.month(), 1)
}

fn get_last_day_in_month_of(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd(date.year(), date.month(), MONTH_2_NDAYS[date.month0() as usize])
}

pub struct TimeLogger {
    file_path: PathBuf,
    date2logday: HashMap<NaiveDate, TimeLogDay>,
}

macro_rules! gen_time_between {
    ($func: ident, $logday_getter: ident, $default_hrs: expr) => {
        fn $func(&self, day1: NaiveDate, day2: NaiveDate, etype: TimeLogEntryType) -> Duration {
            if day1 > day2 {
                return Duration::seconds(0);
            }

            let mut date = day1;
            let mut sum = Duration::seconds(0);
            while date <= day2 {
                if is_weekday(date) {
                    sum = sum + self.date2logday.get(&date).map(|x| x.$logday_getter(etype)).unwrap_or(Duration::hours($default_hrs));
                }
                date = date.succ();
            }

            return sum;
        }
    }
}

macro_rules! gen_x_in_y_of {
    // x is logged/loggable, y is month/week
    ($func: ident, $delegate: ident, $first_day: ident, $last_day: ident) => {
        fn $func(&self, date: NaiveDate, etype: TimeLogEntryType) -> Duration {
            self.$delegate($first_day(date), $last_day(date), etype)
        }
    }
}

macro_rules! gen_todays {
    // start/end_of_day
    ($fname1: ident, $fname2: ident, $delegate: ident) => {
        fn $fname1(&self, date: NaiveDate, etype: TimeLogEntryType) -> Option<NaiveTime> {
            let mut ret = None;
            if let Some(day) = self.date2logday.get(&date) {
                ret = day.$delegate(etype);
            }
            return ret;
        }

        fn $fname2(&self, etype: TimeLogEntryType) -> Option<NaiveTime> {
            self.$fname1(Local::today().naive_local(), etype)
        }
    }
}

macro_rules! gen_log {
    ($fname: ident, $mutator:ident) => {
        pub fn $fname(&mut self, date: NaiveDate, time: NaiveTime) {
            let entry_type = TimeLogEntryType::Work;
            let hms_time = NaiveTime::from_hms(time.hour(), time.minute(), time.second());

            let tld = match self.date2logday.entry(date) {
                Vacant(entry) => entry.insert(TimeLogDay::empty(date)),
                Occupied(entry) => entry.into_mut(),
            };

            tld.$mutator(hms_time, entry_type);
        }
    }
}

const TIMELOGGER_FILE: &str = ".timelog";
impl TimeLogger {

    fn write_entries(&self) -> String {
        let mut s = String::new();
        let mut dates: Vec<&NaiveDate> = self.date2logday.keys().collect();
        dates.sort();
        for date in dates {
            s.push_str(self.date2logday[date].to_string().as_str());
        }

        return s;
    }

    fn read_entries(&mut self, s: &str) -> TimeLogResult<()> {
        let mut lines = s.lines();
        while let Some(line) = lines.next() {
            let tle: TimeLogEntry = line.parse()?;
            let date = tle.date;

            let mut tld = match self.date2logday.entry(date) {
                Vacant(entry) => entry.insert(TimeLogDay::empty(date)),
                Occupied(entry) => entry.into_mut(),
            };

            tld.add_entry(tle);
        }
        Ok(())
    }

    fn from_file(path_buf: PathBuf) -> TimeLogResult<Self> {
        let mut tl = TimeLogger{file_path: path_buf, date2logday: HashMap::new()};
        if !tl.file_path.as_path().exists() {
            File::create(tl.file_path.as_path())?;
        } else {
            let file = File::open(tl.file_path.as_path())?;

            let mut buf_reader = BufReader::new(file);
            let mut contents = String::new();
            buf_reader.read_to_string(&mut contents)?;
            tl.read_entries(contents.as_str())?;
        }

        return Ok(tl);
    }

    pub fn default() -> TimeLogResult<Self> {
        let mut path_buf = std::env::home_dir()
            .ok_or_else(|| TimeLogError::other_io("Can't find home dir"))?;
        path_buf.push(TIMELOGGER_FILE);
        TimeLogger::from_file(path_buf)
    }

    gen_time_between!(compute_logged_time_between, logged_time, 0);
    gen_time_between!(compute_loggable_time_between, loggable_time, 8);

    gen_x_in_y_of!(compute_loggable_time_in_month_of, compute_loggable_time_between,
                  get_first_day_in_month_of, get_last_day_in_month_of);
    gen_x_in_y_of!(compute_logged_time_in_month_of, compute_logged_time_between,
                  get_first_day_in_month_of, get_last_day_in_month_of);
    gen_x_in_y_of!(compute_loggable_time_in_week_of, compute_loggable_time_between,
                  get_first_day_in_week_of, get_last_day_in_week_of);
    gen_x_in_y_of!(compute_logged_time_in_week_of, compute_logged_time_between,
                  get_first_day_in_week_of, get_last_day_in_week_of);

    gen_todays!(start_of_day, todays_start, get_start);
    gen_todays!(end_of_day, todays_end, get_end);

    gen_log!(log_start, set_start);
    gen_log!(log_end, set_end);

    fn compute_time_left_in_month_of(&self, date: NaiveDate, etype: TimeLogEntryType) -> Duration {
        self.compute_loggable_time_in_month_of(date, etype) - self.compute_logged_time_in_month_of(date, etype)
    }

    fn compute_time_left_in_week_of(&self, date: NaiveDate, etype: TimeLogEntryType) -> Duration {
        self.compute_loggable_time_in_week_of(date, etype) - self.compute_logged_time_in_week_of(date, etype)
    }

    fn time_worked_today_with(&self, end: NaiveTime) -> TimeLogResult<Duration> {
        let today = Local::today().naive_local();
        return self.date2logday.get(&today).ok_or_else(|| TimeLogError::inv_inp("Can't find start time, no entries for today\n"))?.time_logged_with(end, TimeLogEntryType::Work);
    }

    fn flextime_as_of(&self, date: NaiveDate) -> Duration {
        let mut keys: Vec<&NaiveDate> = self.date2logday.keys().collect();
        keys.sort();
        if keys.len() == 0 {
            return Duration::hours(0);
        }

        return self.compute_loggable_time_between(*keys[0], date.pred(), TimeLogEntryType::Work) -
            self.compute_logged_time_between(*keys[0], date.pred(), TimeLogEntryType::Work);
    }

    pub fn time_worked_today(&self) -> TimeLogResult<Duration> {
        let etype = TimeLogEntryType::Work;
        let end = self.todays_end(etype).unwrap_or(Local::now().time());
        self.time_worked_today_with(end)
    }

    pub fn time_left_this_week(&self) -> Duration {
        let now = Local::now();
        let etype = TimeLogEntryType::Work;
        let today = now.naive_local().date();
        self.compute_time_left_in_week_of(today, etype) -
            match self.todays_end(etype) {
                Some(_) => Duration::seconds(0), // We have already added this
                None => self.time_worked_today_with(now.time()).unwrap_or(Duration::seconds(0)),
            } +
            self.flextime_as_of(today.pred())
    }

    pub fn hours_left_this_month(&self) -> u32 {
        let today = Local::today().naive_local();
        (self.compute_time_left_in_month_of(today, TimeLogEntryType::Work).num_hours()
            + self.flextime_as_of(today.pred()).num_hours()) as u32
    }

    pub fn total_hours_this_month(&self) -> u32 {
        self.compute_loggable_time_in_month_of(Local::today().naive_local(), TimeLogEntryType::Work).num_hours() as u32
    }

    pub fn save(&self) -> TimeLogResult<()> {
        let mut bkp = self.file_path.clone();
        bkp.set_extension("tl.bkp");
        let bkp_fp = bkp.as_path();
        let fp: &Path = self.file_path.as_path();
        debug_assert!(fp.exists(), "logfile does not exist");
        let mut file = File::create(fp)?;
        let s = self.write_entries();
        fs::copy(fp, bkp_fp)?;
        match file.write_all(s.as_str().as_bytes()) {
            Ok(_) => {
                fs::remove_file(bkp_fp)?;
                return Ok(());
            },
            Err(ref e) => {
                fs::copy(bkp_fp, fp)?;
                return Err(TimeLogError::io_error_extra_msg(e, format!("Failed to write to file (restoring backup): {}", e).as_str()));
            },
        }

    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;
    use chrono::Duration;
    #[test]
    fn timelogentry_basic_mutators() {
        let tdy = Local::today().naive_local();
        let mut entry = TimeLogEntry::new(tdy, TimeLogEntryType::Work);
        let start_time = NaiveTime::from_hms(11, 30, 0);
        entry.set_start(start_time);
        assert_eq!(entry.start, Some(start_time));
        let end_time = NaiveTime::from_hms(12, 30, 0);
        entry.set_end(end_time);
        assert_eq!(entry.end, Some(end_time));
        let entry1 = TimeLogEntry::start(tdy, TimeLogEntryType::Work, start_time);
        assert_eq!(entry1.start, Some(start_time));
        let entry2 = TimeLogEntry::end(tdy, TimeLogEntryType::Work, start_time);
        assert_eq!(entry2.end, Some(start_time));
        let mut entry3 = TimeLogEntry::start(tdy, TimeLogEntryType::Work, start_time);
        let end_time = NaiveTime::from_hms(12, 30, 0);
        entry3.set_end(end_time);
        assert_eq!(entry3.start, Some(start_time));
        assert_eq!(entry3.end, Some(end_time));
    }

    #[test]
    fn timelogentry_from_str() {
        let all_undef = "2017/12/22 Fri | Work UNDEF UNDEF";

        let entry: TimeLogEntry = all_undef.parse().unwrap();
        assert_eq!(entry.start, None);
        assert_eq!(entry.end, None);
        assert_eq!(entry.entry_type, TimeLogEntryType::Work);
        assert_eq!(entry.date, NaiveDate::from_ymd(2017, 12, 22));

        let start = "2017/12/22 Fri | Work 07:31:00 UNDEF";
        let entry: TimeLogEntry = start.parse().unwrap();
        assert_eq!(entry.start, Some(NaiveTime::from_hms(7, 31, 0)));
        assert_eq!(entry.end, None);
        assert_eq!(entry.date, NaiveDate::from_ymd(2017, 12, 22));

        let start_end = "2017/12/22 Fri | Work 07:31:00 12:00:00";
        let start_end_entry: TimeLogEntry = start_end.parse().unwrap();
        assert_eq!(start_end_entry.start, Some(NaiveTime::from_hms(7, 31, 0)));
        assert_eq!(start_end_entry.end, Some(NaiveTime::from_hms(12, 0, 0)));
        assert_eq!(start_end_entry.date, NaiveDate::from_ymd(2017, 12, 22));

        let all_undef_vac: TimeLogEntry = "2017/12/22 Fri | Vacation UNDEF UNDEF".parse().unwrap();
        assert_eq!(all_undef_vac.entry_type, TimeLogEntryType::Vacation);
        assert_eq!(all_undef_vac.date, NaiveDate::from_ymd(2017, 12, 22));
        let all_undef_pl: TimeLogEntry = "2017/12/22 Fri | ParentalLeave UNDEF UNDEF".parse().unwrap();
        assert_eq!(all_undef_pl.entry_type, TimeLogEntryType::ParentalLeave);
        assert_eq!(all_undef_pl.date, NaiveDate::from_ymd(2017, 12, 22));
        let all_undef_s: TimeLogEntry = "2017/12/22 Fri | Sickness UNDEF UNDEF".parse().unwrap();
        assert_eq!(all_undef_s.entry_type, TimeLogEntryType::Sickness);
        assert_eq!(all_undef_s.date, NaiveDate::from_ymd(2017, 12, 22));
    }

    #[test]
    fn timelogentry_consistent_serialiation() {
        let all_undef = "2017/12/22 Fri | Work UNDEF UNDEF";
        assert_eq!(all_undef, all_undef.parse::<TimeLogEntry>().unwrap().to_string());

        let start = "2017/12/22 Fri | Work 07:31:00 UNDEF";
        assert_eq!(start, start.parse::<TimeLogEntry>().unwrap().to_string());

        let start_end = "2017/12/22 Fri | Work 07:31:00 12:00:00";
        assert_eq!(start_end, start_end.parse::<TimeLogEntry>().unwrap().to_string());

        let all_undef_vac = "2017/12/22 Fri | Vacation UNDEF UNDEF";
        assert_eq!(all_undef_vac, all_undef_vac.parse::<TimeLogEntry>().unwrap().to_string());
        let all_undef_pl = "2017/12/22 Fri | ParentalLeave UNDEF UNDEF";
        assert_eq!(all_undef_pl, all_undef_pl.parse::<TimeLogEntry>().unwrap().to_string());
        let all_undef_s = "2017/12/22 Fri | Sickness UNDEF UNDEF";
        assert_eq!(all_undef_s, all_undef_s.parse::<TimeLogEntry>().unwrap().to_string());
    }


    #[test]
    fn timelogday_is_weekday() {
        let mon = TimeLogDay::empty(NaiveDate::from_ymd(2017, 11, 20));
        let tue = TimeLogDay::empty(NaiveDate::from_ymd(2017, 11, 21));
        let wed = TimeLogDay::empty(NaiveDate::from_ymd(2017, 11, 22));
        let thu = TimeLogDay::empty(NaiveDate::from_ymd(2017, 11, 23));
        let fri = TimeLogDay::empty(NaiveDate::from_ymd(2017, 11, 24));
        let sat = TimeLogDay::empty(NaiveDate::from_ymd(2017, 11, 25));
        let sun = TimeLogDay::empty(NaiveDate::from_ymd(2017, 11, 26));
        assert!(mon.is_weekday());
        assert!(tue.is_weekday());
        assert!(wed.is_weekday());
        assert!(thu.is_weekday());
        assert!(fri.is_weekday());
        assert!(!sun.is_weekday());
        assert!(!sat.is_weekday());
    }

    #[test]
    fn timelogday_mutators() {
        let mut mon = TimeLogDay::empty(NaiveDate::from_ymd(2017, 11, 20));
        assert_eq!(mon.date, NaiveDate::from_ymd(2017, 11, 20));
        let start = NaiveTime::from_hms(3, 14, 00);
        mon.set_start(start, TimeLogEntryType::Work);
        assert_eq!(mon.entries[0].start, Some(start));
        assert_eq!(mon.entries[0].end, None);
        assert_eq!(mon.entries.len(), 1);

    }

    #[test]
    fn timelogday_from_str() {
        let all_undef = "2017/12/18 Mon | Work 06:31:00 07:00:00";
        let start = "2017/12/18 Mon | Work 07:31:00 UNDEF";
        let all_undef_vac = "2017/12/18 Mon | Vacation UNDEF UNDEF";
        let all_undef_pl = "2017/12/18 Mon | ParentalLeave UNDEF UNDEF";
        let all_undef_s = "2017/12/18 Mon | Sickness UNDEF UNDEF";
        let s = format!("{}\n{}\n{}\n{}\n{}", all_undef, start,
                        all_undef_vac, all_undef_pl, all_undef_s);

        let day: TimeLogDay = s.parse().unwrap();
        assert_eq!(day.entries.len(), 5);

        assert_eq!(day.entries[0].start, Some(NaiveTime::from_hms(6, 31, 0)));
        assert_eq!(day.entries[0].end, Some(NaiveTime::from_hms(7, 0, 0)));
        assert_eq!(day.entries[0].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[0].date, NaiveDate::from_ymd(2017, 12, 18));

        assert_eq!(day.entries[1].start, Some(NaiveTime::from_hms(7, 31, 0)));
        assert_eq!(day.entries[1].end, None);
        assert_eq!(day.entries[1].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[1].date, NaiveDate::from_ymd(2017, 12, 18));

        assert_eq!(day.entries[2].start, None);
        assert_eq!(day.entries[2].end, None);
        assert_eq!(day.entries[2].entry_type, TimeLogEntryType::Vacation);
        assert_eq!(day.entries[2].date, NaiveDate::from_ymd(2017, 12, 18));

        assert_eq!(day.entries[3].start, None);
        assert_eq!(day.entries[3].end, None);
        assert_eq!(day.entries[3].entry_type, TimeLogEntryType::ParentalLeave);
        assert_eq!(day.entries[3].date, NaiveDate::from_ymd(2017, 12, 18));

        assert_eq!(day.entries[4].start, None);
        assert_eq!(day.entries[4].end, None);
        assert_eq!(day.entries[4].entry_type, TimeLogEntryType::Sickness);
        assert_eq!(day.entries[4].date, NaiveDate::from_ymd(2017, 12, 18));
        assert_eq!(day.date, NaiveDate::from_ymd(2017, 12, 18));
    }

    #[test]
    fn timelogday_consistent_serialization() {
        let all_undef = "2017/12/18 Mon | Work 06:31:00 07:00:00";
        let start = "2017/12/18 Mon | Work 07:31:00 UNDEF";
        let all_undef_vac = "2017/12/18 Mon | Vacation UNDEF UNDEF";
        let all_undef_pl = "2017/12/18 Mon | ParentalLeave UNDEF UNDEF";
        let all_undef_s = "2017/12/18 Mon | Sickness UNDEF UNDEF";
        let s = format!("{}\n{}\n{}\n{}\n{}", all_undef, start,
                        all_undef_vac, all_undef_pl, all_undef_s);

        let day: TimeLogDay = s.parse().unwrap();
        assert_eq!(day.entries.len(), 5);

        assert_eq!(day.entries[0].start, Some(NaiveTime::from_hms(6, 31, 0)));
        assert_eq!(day.entries[0].end, Some(NaiveTime::from_hms(7, 0, 0)));
        assert_eq!(day.entries[0].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[0].date, NaiveDate::from_ymd(2017, 12, 18));

        assert_eq!(day.entries[1].start, Some(NaiveTime::from_hms(7, 31, 0)));
        assert_eq!(day.entries[1].end, None);
        assert_eq!(day.entries[1].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[1].date, NaiveDate::from_ymd(2017, 12, 18));

        assert_eq!(day.entries[2].start, None);
        assert_eq!(day.entries[2].end, None);
        assert_eq!(day.entries[2].entry_type, TimeLogEntryType::Vacation);
        assert_eq!(day.entries[2].date, NaiveDate::from_ymd(2017, 12, 18));

        assert_eq!(day.entries[3].start, None);
        assert_eq!(day.entries[3].end, None);
        assert_eq!(day.entries[3].entry_type, TimeLogEntryType::ParentalLeave);
        assert_eq!(day.entries[3].date, NaiveDate::from_ymd(2017, 12, 18));

        assert_eq!(day.entries[4].start, None);
        assert_eq!(day.entries[4].end, None);
        assert_eq!(day.entries[4].entry_type, TimeLogEntryType::Sickness);
        assert_eq!(day.entries[4].date, NaiveDate::from_ymd(2017, 12, 18));
        assert_eq!(day.date, NaiveDate::from_ymd(2017, 12, 18));

        assert_eq!(day.to_string(), s);
    }

    #[test]
    fn timelogday_basic_mutators() {
        let entries = vec![
            "2017/12/18 Mon | Work UNDEF 07:00:00\n",
            "2017/12/18 Mon | Work 07:31:00 UNDEF\n",
            "2017/12/18 Mon | Work UNDEF UNDEF\n"];
        let mut s = String::new();
        for e in entries {
            s.push_str(e);
        }

        let mut day: TimeLogDay = s.as_str().parse().unwrap();

        day.set_start(NaiveTime::from_hms(06,30,00), TimeLogEntryType::Work);
        assert_eq!(day.entries[0].start, Some(NaiveTime::from_hms(6, 30, 0)));
        assert_eq!(day.entries[0].end, Some(NaiveTime::from_hms(7, 0, 0)));
        assert_eq!(day.entries[0].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[0].date, NaiveDate::from_ymd(2017, 12, 18));

        day.set_start(NaiveTime::from_hms(12,30,00), TimeLogEntryType::Work);
        assert_eq!(day.entries[2].start, Some(NaiveTime::from_hms(12, 30, 0)));
        assert_eq!(day.entries[2].end, None);
        assert_eq!(day.entries[2].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[2].date, NaiveDate::from_ymd(2017, 12, 18));

        day.set_end(NaiveTime::from_hms(12,25,00), TimeLogEntryType::Work);
        assert_eq!(day.entries[1].start, Some(NaiveTime::from_hms(7,31,0)));
        assert_eq!(day.entries[1].end, Some(NaiveTime::from_hms(12, 25, 0)));
        assert_eq!(day.entries[1].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[1].date, NaiveDate::from_ymd(2017, 12, 18));

        day.set_end(NaiveTime::from_hms(19,12,00), TimeLogEntryType::Work);
        assert_eq!(day.entries[2].start, Some(NaiveTime::from_hms(12,30,0)));
        assert_eq!(day.entries[2].end, Some(NaiveTime::from_hms(19, 12, 0)));
        assert_eq!(day.entries[2].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[2].date, NaiveDate::from_ymd(2017, 12, 18));
    }

    #[test]
    fn timelogday_diff_entry_types() {
        let entries = vec![
            "2017/12/18 Mon | Work UNDEF 07:00:00\n",
            "2017/12/18 Mon | Sickness 07:31:00 UNDEF"];
        let mut s = String::new();
        for e in entries {
            s.push_str(e);
        }

        let mut day: TimeLogDay = s.as_str().parse().unwrap();

        day.set_start(NaiveTime::from_hms(06,30,00), TimeLogEntryType::Work);
        assert_eq!(day.entries[0].start, Some(NaiveTime::from_hms(6, 30, 0)));
        assert_eq!(day.entries[0].end, Some(NaiveTime::from_hms(7, 0, 0)));
        assert_eq!(day.entries[0].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[0].date, NaiveDate::from_ymd(2017, 12, 18));

        day.set_start(NaiveTime::from_hms(12,30,00), TimeLogEntryType::Sickness);
        assert_eq!(day.entries[2].start, Some(NaiveTime::from_hms(12, 30, 0)));
        assert_eq!(day.entries[2].end, None);
        assert_eq!(day.entries[2].entry_type, TimeLogEntryType::Sickness);
        assert_eq!(day.entries[2].date, NaiveDate::from_ymd(2017, 12, 18));

        day.set_end(NaiveTime::from_hms(12,25,00), TimeLogEntryType::Sickness);
        assert_eq!(day.entries[1].start, Some(NaiveTime::from_hms(7,31,0)));
        assert_eq!(day.entries[1].end, Some(NaiveTime::from_hms(12, 25, 0)));
        assert_eq!(day.entries[1].entry_type, TimeLogEntryType::Sickness);
        assert_eq!(day.entries[1].date, NaiveDate::from_ymd(2017, 12, 18));

        assert_eq!(day.entries[0].start, Some(NaiveTime::from_hms(6, 30, 0)));
        assert_eq!(day.entries[0].end, Some(NaiveTime::from_hms(7, 0, 0)));
        assert_eq!(day.entries[0].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[0].date, NaiveDate::from_ymd(2017, 12, 18));
        assert_eq!(day.entries[2].start, Some(NaiveTime::from_hms(12, 30, 0)));
        assert_eq!(day.entries[2].end, None);
        assert_eq!(day.entries[2].entry_type, TimeLogEntryType::Sickness);
        assert_eq!(day.entries[2].date, NaiveDate::from_ymd(2017, 12, 18));
    }



    /*
       #[test]
       fn timelogday_from_iterator() {
       let lines = "2017/11/01 Wednesday\nWork 01:01:00 02:02:00\nSickness 03:03:00 04:04:00\nWork 04:05:00 04:06:00".lines();
       let some_entries = TimeLogDay::from_iterator(&mut lines.peekable()).unwrap();
       assert_eq!(some_entries.entries.len(), 3);
       assert_eq!(some_entries.date, NaiveDate::from_ymd(2017, 11, 1));
       assert_eq!(some_entries.entries[0].start, Some(NaiveTime::from_hms(1,1,0)));
       assert_eq!(some_entries.entries[0].end, Some(NaiveTime::from_hms(2,2,0)));
       assert_eq!(some_entries.entries[1].start, Some(NaiveTime::from_hms(3,3,0)));
       assert_eq!(some_entries.entries[1].end, Some(NaiveTime::from_hms(4,4,0)));
       assert_eq!(some_entries.entries[2].start, Some(NaiveTime::from_hms(4,5,0)));
       assert_eq!(some_entries.entries[2].end, Some(NaiveTime::from_hms(4,6,0)));
       }
       */
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
    fn timelogger_read_entries() {
        let mon_1 = "2017/12/18 Mon | Work 06:31:00 07:00:00";
        let mon_2 = "2017/12/18 Mon | Work 07:31:00 UNDEF";
        let tue_1 = "2017/12/19 Tue | Work 07:31:00 11:50:00";
        let tue_2 = "2017/12/19 Tue | Work 12:34:00 18:15:00";
        let wed_1 = "2017/12/20 Wed | Work 09:10:00 11:55:00";
        let wed_2 = "2017/12/20 Wed | Work 12:40:00 18:45:00";

        let s = format!("{}\n{}\n{}\n{}\n{}\n{}",
                        mon_1, mon_2, tue_1, tue_2, wed_1, wed_2);

        let mut logger = TimeLogger{file_path: PathBuf::new(), date2logday: HashMap::new()};
        logger.read_entries(s.as_str()).unwrap();

        let mon = &NaiveDate::from_ymd(2017, 12, 18);
        let tue = &NaiveDate::from_ymd(2017, 12, 19);
        let wed = &NaiveDate::from_ymd(2017, 12, 20);

        assert_eq!(logger.date2logday.len(), 3);
        assert_eq!(logger.date2logday[mon].date, *mon);
        assert_eq!(logger.date2logday[tue].date, *tue);
        assert_eq!(logger.date2logday[wed].date, *wed);

        assert_eq!(logger.date2logday[mon].entries[0].start, Some(NaiveTime::from_hms(6,31,0)));
        assert_eq!(logger.date2logday[mon].entries[0].end, Some(NaiveTime::from_hms(07,00,0)));
        assert_eq!(logger.date2logday[mon].entries[0].entry_type, TimeLogEntryType::Work);
        assert_eq!(logger.date2logday[mon].entries[0].date, *mon);

        assert_eq!(logger.date2logday[mon].entries[1].start, Some(NaiveTime::from_hms(7,31,0)));
        assert_eq!(logger.date2logday[mon].entries[1].end, None);
        assert_eq!(logger.date2logday[mon].entries[1].entry_type, TimeLogEntryType::Work);
        assert_eq!(logger.date2logday[mon].entries[1].date, *mon);

        assert_eq!(logger.date2logday[tue].entries[0].start, Some(NaiveTime::from_hms(7,31,0)));
        assert_eq!(logger.date2logday[tue].entries[0].end, Some(NaiveTime::from_hms(11,50,0)));
        assert_eq!(logger.date2logday[tue].entries[0].entry_type, TimeLogEntryType::Work);
        assert_eq!(logger.date2logday[tue].entries[0].date, *tue);

        assert_eq!(logger.date2logday[tue].entries[1].start, Some(NaiveTime::from_hms(12,34,0)));
        assert_eq!(logger.date2logday[tue].entries[1].end, Some(NaiveTime::from_hms(18,15,0)));
        assert_eq!(logger.date2logday[tue].entries[1].entry_type, TimeLogEntryType::Work);
        assert_eq!(logger.date2logday[tue].entries[1].date, *tue);

        assert_eq!(logger.date2logday[wed].entries[0].start, Some(NaiveTime::from_hms(9,10,0)));
        assert_eq!(logger.date2logday[wed].entries[0].end, Some(NaiveTime::from_hms(11,55,0)));
        assert_eq!(logger.date2logday[wed].entries[0].entry_type, TimeLogEntryType::Work);
        assert_eq!(logger.date2logday[wed].entries[0].date, *wed);

        assert_eq!(logger.date2logday[wed].entries[1].start, Some(NaiveTime::from_hms(12,40,0)));
        assert_eq!(logger.date2logday[wed].entries[1].end, Some(NaiveTime::from_hms(18,45,0)));
        assert_eq!(logger.date2logday[wed].entries[1].entry_type, TimeLogEntryType::Work);
        assert_eq!(logger.date2logday[wed].entries[1].date, *wed);
    }

    #[test]
    fn timelogger_compute_logged_time_between() {
        let mon_1 = "2017/12/18 Mon | Work 06:31:00 07:00:00";
        let mon_2 = "2017/12/18 Mon | Work 07:31:00 UNDEF";
        let tue_1 = "2017/12/19 Tue | Work 07:31:00 11:50:00";
        let tue_2 = "2017/12/19 Tue | Work 12:34:00 18:15:00";
        let wed_1 = "2017/12/20 Wed | Work 09:10:00 11:55:00";
        let wed_2 = "2017/12/20 Wed | Work 12:40:00 18:45:00";

        let s = format!("{}\n{}\n{}\n{}\n{}\n{}",
                        mon_1, mon_2, tue_1, tue_2, wed_1, wed_2);

        let mut logger = TimeLogger{file_path: PathBuf::new(), date2logday: HashMap::new()};
        logger.read_entries(s.as_str()).unwrap();

        let mon = NaiveDate::from_ymd(2017,12,18);
        let tue = NaiveDate::from_ymd(2017,12,19);
        let wed = NaiveDate::from_ymd(2017,12,20);
        let dur_mon = Duration::minutes(29);
        let dur_tue = Duration::minutes(4 * 60 + 19 + 5 * 60 + 41);
        let dur_wed = Duration::minutes(2 * 60  + 45 + 6 * 60 + 5);
        assert_eq!(logger.compute_logged_time_between(mon, mon, TimeLogEntryType::Work), dur_mon);
        assert_eq!(logger.compute_logged_time_between(tue, tue, TimeLogEntryType::Work), dur_tue);
        assert_eq!(logger.compute_logged_time_between(wed, wed, TimeLogEntryType::Work), dur_wed);
        assert_eq!(logger.compute_logged_time_between(mon, tue, TimeLogEntryType::Work), dur_mon + dur_tue);
        assert_eq!(logger.compute_logged_time_between(tue, wed, TimeLogEntryType::Work), dur_tue + dur_wed);
        assert_eq!(logger.compute_logged_time_between(mon, wed, TimeLogEntryType::Work), dur_mon + dur_tue + dur_wed);
    }

    #[test]
    fn timelogger_compute_loggable_time_between() {
        let mon_1 = "2017/12/18 Mon | Work 06:31:00 07:00:00";
        let mon_2 = "2017/12/18 Mon | Work 07:31:00 UNDEF";
        let tue_1 = "2017/12/19 Tue | Work 07:31:00 11:50:00";
        let tue_2 = "2017/12/19 Tue | Work 12:34:00 18:15:00";
        let wed_1 = "2017/12/20 Wed | Work 09:10:00 11:55:00";
        let wed_2 = "2017/12/20 Wed | Work 12:40:00 18:45:00";

        let s = format!("{}\n{}\n{}\n{}\n{}\n{}",
                        mon_1, mon_2, tue_1, tue_2, wed_1, wed_2);

        let mut logger = TimeLogger{file_path: PathBuf::new(), date2logday: HashMap::new()};
        logger.read_entries(s.as_str()).unwrap();

        let prev_fri = NaiveDate::from_ymd(2017,12,15);
        let sat = NaiveDate::from_ymd(2017,12,16);
        let sun = NaiveDate::from_ymd(2017,12,17);
        let mon = NaiveDate::from_ymd(2017,12,18);
        let tue = NaiveDate::from_ymd(2017,12,19);
        let wed = NaiveDate::from_ymd(2017,12,20);
        let thu = NaiveDate::from_ymd(2017,12,21);
        let fri = NaiveDate::from_ymd(2017,12,22);

        let d0hr = Duration::hours(0);
        let d8hr = Duration::hours(8);
        let d16hr = Duration::hours(16);
        let d24hr = Duration::hours(24);
        let d32hr = Duration::hours(32);
        let d40hr = Duration::hours(40);
        let d48hr = Duration::hours(48);

        assert_eq!(logger.compute_loggable_time_between(mon, mon, TimeLogEntryType::Work), d8hr);
        assert_eq!(logger.compute_loggable_time_between(tue, tue, TimeLogEntryType::Work), d8hr);
        assert_eq!(logger.compute_loggable_time_between(wed, wed, TimeLogEntryType::Work), d8hr);
        assert_eq!(logger.compute_loggable_time_between(thu, thu, TimeLogEntryType::Work), d8hr);
        assert_eq!(logger.compute_loggable_time_between(fri, fri, TimeLogEntryType::Work), d8hr);
        assert_eq!(logger.compute_loggable_time_between(sat, sat, TimeLogEntryType::Work), d0hr);
        assert_eq!(logger.compute_loggable_time_between(sun, sun, TimeLogEntryType::Work), d0hr);

        assert_eq!(logger.compute_loggable_time_between(mon, tue, TimeLogEntryType::Work), d16hr);
        assert_eq!(logger.compute_loggable_time_between(tue, wed, TimeLogEntryType::Work), d16hr);
        assert_eq!(logger.compute_loggable_time_between(wed, thu, TimeLogEntryType::Work), d16hr);
        assert_eq!(logger.compute_loggable_time_between(thu, fri, TimeLogEntryType::Work), d16hr);

        assert_eq!(logger.compute_loggable_time_between(mon, wed, TimeLogEntryType::Work), d24hr);
        assert_eq!(logger.compute_loggable_time_between(mon, thu, TimeLogEntryType::Work), d32hr);
        assert_eq!(logger.compute_loggable_time_between(mon, fri, TimeLogEntryType::Work), d40hr);

        assert_eq!(logger.compute_loggable_time_between(sun, mon, TimeLogEntryType::Work), d8hr);
        assert_eq!(logger.compute_loggable_time_between(sun, tue, TimeLogEntryType::Work), d16hr);
        assert_eq!(logger.compute_loggable_time_between(sat, mon, TimeLogEntryType::Work), d8hr);
        assert_eq!(logger.compute_loggable_time_between(sat, tue, TimeLogEntryType::Work), d16hr);

        assert_eq!(logger.compute_loggable_time_between(prev_fri, sat, TimeLogEntryType::Work), d8hr);
        assert_eq!(logger.compute_loggable_time_between(prev_fri, sun, TimeLogEntryType::Work), d8hr);
        assert_eq!(logger.compute_loggable_time_between(prev_fri, mon, TimeLogEntryType::Work), d16hr);
        assert_eq!(logger.compute_loggable_time_between(prev_fri, tue, TimeLogEntryType::Work), d24hr);
        assert_eq!(logger.compute_loggable_time_between(prev_fri, wed, TimeLogEntryType::Work), d32hr);
        assert_eq!(logger.compute_loggable_time_between(prev_fri, thu, TimeLogEntryType::Work), d40hr);
        assert_eq!(logger.compute_loggable_time_between(prev_fri, fri, TimeLogEntryType::Work), d48hr);
    }

    #[test]
    fn timelogger_x_in_y_of() {
        let nov_mon_1 = "2017/11/13 Mon | Work 08:00:00 18:00:00";
        let nov_tue_1 = "2017/11/14 Tue | Work 07:30:00 12:00:00";
        let nov_wed_1 = "2017/11/15 Wed | Work 09:10:00 15:10:00";
        let mon_1 = "2017/12/18 Mon | Work 06:31:00 07:00:00";
        let mon_2 = "2017/12/18 Mon | Work 07:31:00 UNDEF";
        let tue_1 = "2017/12/19 Tue | Work 07:31:00 11:50:00";
        let tue_2 = "2017/12/19 Tue | Work 12:34:00 18:15:00";
        let wed_1 = "2017/12/20 Wed | Work 09:10:00 11:55:00";
        let wed_2 = "2017/12/20 Wed | Work 12:40:00 18:45:00";

        let s = format!("{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
                        nov_mon_1, nov_tue_1, nov_wed_1,
                        mon_1, mon_2, tue_1, tue_2, wed_1, wed_2);

        let mut logger = TimeLogger{file_path: PathBuf::new(), date2logday: HashMap::new()};
        logger.read_entries(s.as_str()).unwrap();

        let prev_fri = NaiveDate::from_ymd(2017,12,15);
        let sat = NaiveDate::from_ymd(2017,12,16);
        let sun = NaiveDate::from_ymd(2017,12,17);
        let mon = NaiveDate::from_ymd(2017,12,18);
        let tue = NaiveDate::from_ymd(2017,12,19);
        let wed = NaiveDate::from_ymd(2017,12,20);
        let thu = NaiveDate::from_ymd(2017,12,21);
        let fri = NaiveDate::from_ymd(2017,12,22);

        let nov_mon = NaiveDate::from_ymd(2017,11,13);
        let nov_tue = NaiveDate::from_ymd(2017,11,14);
        let nov_wed = NaiveDate::from_ymd(2017,11,15);

        let d0hr = Duration::hours(0);
        let d40hr = Duration::hours(40);

        let dur_mon = Duration::minutes(29);
        let dur_tue = Duration::minutes(4 * 60 + 19 + 5 * 60 + 41);
        let dur_wed = Duration::minutes(2 * 60  + 45 + 6 * 60 + 5);
        let dur_tot = dur_mon + dur_tue + dur_wed;
        let dur_nov = Duration::minutes(10 * 60 + 4 * 60 + 30 + 6 * 60);

        assert_eq!(logger.compute_loggable_time_in_week_of(prev_fri, TimeLogEntryType::Work), d40hr);
        assert_eq!(logger.compute_loggable_time_in_week_of(sat, TimeLogEntryType::Work), d40hr);
        assert_eq!(logger.compute_loggable_time_in_week_of(sun, TimeLogEntryType::Work), d40hr);
        assert_eq!(logger.compute_loggable_time_in_week_of(mon, TimeLogEntryType::Work), d40hr);
        assert_eq!(logger.compute_loggable_time_in_week_of(tue, TimeLogEntryType::Work), d40hr);
        assert_eq!(logger.compute_loggable_time_in_week_of(wed, TimeLogEntryType::Work), d40hr);
        assert_eq!(logger.compute_loggable_time_in_week_of(thu, TimeLogEntryType::Work), d40hr);
        assert_eq!(logger.compute_loggable_time_in_week_of(fri, TimeLogEntryType::Work), d40hr);

        assert_eq!(logger.compute_loggable_time_in_month_of(sun, TimeLogEntryType::Work), Duration::hours(168));
        assert_eq!(logger.compute_loggable_time_in_month_of(mon, TimeLogEntryType::Work), Duration::hours(168));
        assert_eq!(logger.compute_loggable_time_in_month_of(nov_mon, TimeLogEntryType::Work), Duration::hours(176));
		assert_eq!(logger.compute_logged_time_in_week_of(prev_fri, TimeLogEntryType::Work), d0hr);
        assert_eq!(logger.compute_logged_time_in_week_of(sat, TimeLogEntryType::Work), d0hr);
        assert_eq!(logger.compute_logged_time_in_week_of(sun, TimeLogEntryType::Work), d0hr);
        assert_eq!(logger.compute_logged_time_in_week_of(mon, TimeLogEntryType::Work), dur_tot);
        assert_eq!(logger.compute_logged_time_in_week_of(tue, TimeLogEntryType::Work), dur_tot);
        assert_eq!(logger.compute_logged_time_in_week_of(wed, TimeLogEntryType::Work), dur_tot);
        assert_eq!(logger.compute_logged_time_in_week_of(thu, TimeLogEntryType::Work), dur_tot);
        assert_eq!(logger.compute_logged_time_in_week_of(fri, TimeLogEntryType::Work), dur_tot);

        assert_eq!(logger.compute_logged_time_in_week_of(nov_mon, TimeLogEntryType::Work), dur_nov);
        assert_eq!(logger.compute_logged_time_in_week_of(nov_tue, TimeLogEntryType::Work), dur_nov);
        assert_eq!(logger.compute_logged_time_in_week_of(nov_wed, TimeLogEntryType::Work), dur_nov);

        assert_eq!(logger.compute_logged_time_in_month_of(nov_mon, TimeLogEntryType::Work), dur_nov);
        assert_eq!(logger.compute_logged_time_in_month_of(nov_tue, TimeLogEntryType::Work), dur_nov);
        assert_eq!(logger.compute_logged_time_in_month_of(nov_wed, TimeLogEntryType::Work), dur_nov);

        assert_eq!(logger.compute_logged_time_in_month_of(mon, TimeLogEntryType::Work), dur_tot);
        assert_eq!(logger.compute_logged_time_in_month_of(tue, TimeLogEntryType::Work), dur_tot);
        assert_eq!(logger.compute_logged_time_in_month_of(wed, TimeLogEntryType::Work), dur_tot);
    }

    #[test]
    fn timelogger_log_start_end() {
        let mut logger = TimeLogger{file_path: PathBuf::new(), date2logday: HashMap::new()};
        let today = &Local::today().naive_local();
        let start = NaiveTime::from_hms(12,0,0);
        let end = NaiveTime::from_hms(13,0,0);
        logger.log_start(*today, start);
        logger.log_end(*today, end);
        assert_eq!(logger.date2logday[today].entries[0].start, Some(start));
        assert_eq!(logger.date2logday[today].entries[0].end, Some(end));
    }

    #[test]
    fn timelogger_todays_start_end() {
        let mon_1 = "2017/12/18 Mon | Work 06:31:00 07:00:00";
        let mon_2 = "2017/12/18 Mon | Work 07:31:00 UNDEF";
        let tue_1 = "2017/12/19 Tue | Work UNDEF 11:50:00";
        let tue_2 = "2017/12/19 Tue | Work 12:31:00 18:15:00";
        let wed_1 = "2017/12/20 Wed | Work UNDEF UNDEF";
        let wed_2 = "2017/12/20 Wed | Work UNDEF UNDEF";

        let s = format!("{}\n{}\n{}\n{}\n{}\n{}",
                        mon_1, mon_2, tue_1, tue_2, wed_1, wed_2);

        let mut logger = TimeLogger{file_path: PathBuf::new(), date2logday: HashMap::new()};
        logger.read_entries(s.as_str()).unwrap();

        let mon = NaiveDate::from_ymd(2017,12,18);
        let tue = NaiveDate::from_ymd(2017,12,19);
        let wed = NaiveDate::from_ymd(2017,12,20);

        assert_eq!(logger.start_of_day(mon, TimeLogEntryType::Work), Some(NaiveTime::from_hms(6,31,0)));
        assert_eq!(logger.start_of_day(tue, TimeLogEntryType::Work), Some(NaiveTime::from_hms(12,31,0)));
        assert_eq!(logger.start_of_day(wed, TimeLogEntryType::Work), None);

        assert_eq!(logger.end_of_day(mon, TimeLogEntryType::Work), Some(NaiveTime::from_hms(7,0,0)));
        assert_eq!(logger.end_of_day(tue, TimeLogEntryType::Work), Some(NaiveTime::from_hms(18,15,0)));
        assert_eq!(logger.end_of_day(wed, TimeLogEntryType::Work), None);
    }

    #[test]
    fn timelogger_flex_time() {
        let mon_1 = "2017/12/18 Mon | Work 08:00:00 16:00:00";
        let tue_1 = "2017/12/19 Tue | Work 09:00:00 16:00:00";
        let wed_1 = "2017/12/20 Wed | Work 08:00:00 16:00:00";
        let thu_1 = "2017/12/21 Thu | Work 10:00:00 17:00:00";
        let fri_1 = "2017/12/22 Fri | Work 08:00:00 16:45:00";

        let s = format!("{}\n{}\n{}\n{}\n{}",
                        mon_1, tue_1, wed_1, thu_1, fri_1);

        let mut logger = TimeLogger{file_path: PathBuf::new(), date2logday: HashMap::new()};
        logger.read_entries(s.as_str()).unwrap();

        let mon = NaiveDate::from_ymd(2017,12,18);
        let tue = NaiveDate::from_ymd(2017,12,19);
        let wed = NaiveDate::from_ymd(2017,12,20);
        let thu = NaiveDate::from_ymd(2017,12,21);
        let fri = NaiveDate::from_ymd(2017,12,22);
        let sat = NaiveDate::from_ymd(2017,12,23);

        assert_eq!(logger.flextime_as_of(mon), Duration::hours(0));
        assert_eq!(logger.flextime_as_of(tue), Duration::hours(0));
        assert_eq!(logger.flextime_as_of(wed), Duration::hours(1));
        assert_eq!(logger.flextime_as_of(thu), Duration::hours(1));
        assert_eq!(logger.flextime_as_of(fri), Duration::hours(2));
        assert_eq!(logger.flextime_as_of(sat), Duration::minutes(60 + 15));

    }
}
