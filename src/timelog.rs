use std::cmp::Ordering;
use std::error::Error;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::io;
use std::slice::Iter;
use std::str::FromStr;

use chrono::prelude::*;
use chrono::Duration;
use chrono::NaiveTime;
use chrono::Weekday;

use crate::TimeLogError::{IOError, InvalidInputError, ParseError, TimeError};

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

    pub fn inv_inp(s: &str) -> TimeLogError {
        TimeLogError::InvalidInputError(String::from(s))
    }

    pub fn io_error_extra_msg(e: &io::Error, msg: &str) -> TimeLogError {
        TimeLogError::IOError(std::io::Error::new(e.kind(), msg))
    }

    pub fn other_io(msg: &str) -> TimeLogError {
        TimeLogError::IOError(std::io::Error::new(io::ErrorKind::Other, msg))
    }
}

impl Error for TimeLogError {}
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

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub enum TimeLogEntryType {
    Work,
    Holiday,
    Sickness,
    Vacation,
    ParentalLeave,
}

impl TimeLogEntryType {
    const ETYPES: [TimeLogEntryType; 5] = [
        TimeLogEntryType::Work,
        TimeLogEntryType::Sickness,
        TimeLogEntryType::Vacation,
        TimeLogEntryType::ParentalLeave,
        TimeLogEntryType::Holiday,
    ];

    pub fn iterator() -> Iter<'static, TimeLogEntryType> {
        Self::ETYPES.iter()
    }
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
            "Holiday" => Ok(TimeLogEntryType::Holiday),
            _ => Err(TimeLogError::parse_error(format!(
                "Can't parse: {} as TimeLogEntryType",
                s
            ))),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TimeLogEntry {
    start: Option<NaiveTime>,
    end: Option<NaiveTime>,
    entry_type: TimeLogEntryType,
    date: NaiveDate,
}

impl Ord for TimeLogEntry {
    fn cmp(&self, other: &TimeLogEntry) -> Ordering {
        match (
            self.start,
            self.end,
            self.entry_type,
            self.date,
            other.start,
            other.end,
            other.entry_type,
            other.date,
        ) {
            (_, Some(end), _, _, Some(start), _, _, _) => end.cmp(&start),
            (Some(start), _, _, _, _, Some(end), _, _) => start.cmp(&end),
            (Some(start0), _, _, _, Some(start1), _, _, _) => start0.cmp(&start1),
            (_, Some(end0), _, _, _, Some(end1), _, _) => end0.cmp(&end1),
            (_, _, et0, _, _, _, et1, _) => et0.cmp(&et1),
        }
    }
}

impl PartialOrd for TimeLogEntry {
    fn partial_cmp(&self, other: &TimeLogEntry) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl TimeLogEntry {
    fn start(date: NaiveDate, entry_type: TimeLogEntryType, time: NaiveTime) -> Self {
        TimeLogEntry {
            date,
            entry_type,
            start: Some(time),
            end: None,
        }
    }

    fn end(date: NaiveDate, entry_type: TimeLogEntryType, time: NaiveTime) -> Self {
        TimeLogEntry {
            date,
            entry_type,
            start: None,
            end: Some(time),
        }
    }

    fn set_start(&mut self, time: NaiveTime) {
        debug_assert!(time.nanosecond() == 0);
        self.start = Some(time);
    }

    fn set_end(&mut self, time: NaiveTime) {
        debug_assert!(time.nanosecond() == 0);
        self.end = Some(time);
    }

    pub fn get_date(&self) -> NaiveDate {
        self.date
    }
}

fn try_get_naivetime(s: &str) -> Option<NaiveTime> {
    if s.contains("UNDEF") {
        None
    } else {
        NaiveTime::from_str(s).ok()
    }
}

const TIMELOGENTRY_NAIVEDATE_FORMAT_STRING: &str = "%Y/%m/%d %a";

impl FromStr for TimeLogEntry {
    type Err = TimeLogError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut delim_split = s.split('|');

        let date: NaiveDate = NaiveDate::parse_from_str(
            delim_split
                .next()
                .ok_or_else(|| TimeLogError::parse_error(format!("Can't read date from: {}", s)))?
                .trim(),
            TIMELOGENTRY_NAIVEDATE_FORMAT_STRING,
        )?;

        let mut space_split = delim_split
            .next()
            .ok_or_else(|| TimeLogError::parse_error(format!("Invalid format for entry: {}", s)))?
            .trim()
            .split(' ');

        let entry_type: TimeLogEntryType = space_split
            .next()
            .ok_or_else(|| TimeLogError::parse_error(format!("Can't read type from: {}", s)))?
            .trim()
            .parse()?;

        let start = try_get_naivetime(
            space_split
                .next()
                .ok_or_else(|| TimeLogError::parse_error(format!("Can't read start from: {}", s)))?
                .trim(),
        );
        let end = try_get_naivetime(
            space_split
                .next()
                .ok_or_else(|| TimeLogError::parse_error(format!("Can't read end from: {}", s)))?
                .trim(),
        );
        Ok(TimeLogEntry {
            date,
            entry_type,
            start,
            end,
        })
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
        write!(
            f,
            "{} | {} {} {}",
            self.date.format(TIMELOGENTRY_NAIVEDATE_FORMAT_STRING),
            self.entry_type,
            opt_naivetime_to_str(self.start),
            opt_naivetime_to_str(self.end)
        )
    }
}

#[derive(Debug, PartialEq)]
pub struct TimeLogDay {
    date: NaiveDate,
    entries: Vec<TimeLogEntry>,
}

impl From<TimeLogEntry> for TimeLogDay {
    fn from(entry: TimeLogEntry) -> TimeLogDay {
        let mut v = Vec::new();
        v.push(entry);
        TimeLogDay {
            date: entry.date,
            entries: v,
        }
    }
}

macro_rules! gen_set {
    ($func: ident, $entry_field: ident, $entry_mutator: ident, $ctor: path) => {
        pub fn $func(&mut self, time: NaiveTime, entry_type: TimeLogEntryType) {
            debug_assert!(self.validate_ordering());
            debug_assert!(time.nanosecond() == 0);
            let mut found = false;
            for entry in &mut self.entries {
                if entry.$entry_field.is_none() && entry.entry_type == entry_type {
                    if found {
                        println!("WARN: Found more than one UNDEF this day");
                        break;
                    } else {
                        entry.$entry_mutator(time);
                        found = true;
                    }
                }
            }

            if !found {
                self.entries.push($ctor(self.date, entry_type, time));
            }
            self.entries.sort();
        }
    };
}

pub fn is_weekday(date: NaiveDate) -> bool {
    date.weekday() != Weekday::Sat && date.weekday() != Weekday::Sun
}

fn is_workday(date: NaiveDate) -> bool {
    is_weekday(date)
}

impl TimeLogDay {
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
        true
    }

    pub fn add_entry(&mut self, e: TimeLogEntry) {
        self.entries.push(e);
        self.entries.sort();
    }

    pub fn empty(date: NaiveDate) -> TimeLogDay {
        TimeLogDay {
            date,
            entries: Vec::new(),
        }
    }

    pub fn full(date: NaiveDate, entry_type: TimeLogEntryType) -> Self {
        let entries = vec![TimeLogEntry {
            date,
            entry_type,
            start: None,
            end: None,
        }];
        TimeLogDay { date, entries }
    }

    gen_set!(set_end, end, set_end, TimeLogEntry::end);
    gen_set!(set_start, start, set_start, TimeLogEntry::start);

    pub fn time_logged_with(
        &self,
        with: Option<NaiveTime>,
        etype: TimeLogEntryType,
    ) -> TimeLogResult<Duration> {
        let mut dur = self.logged_time(etype);
        if with.is_none() {
            return Ok(dur);
        }

        if self.entries.is_empty() {
            return Err(TimeLogError::inv_inp("No entries today"));
        } else if self.entries.iter().all(|e| e.start.is_none()) {
            return Err(TimeLogError::inv_inp("No start entries today"));
        }

        // If we find an an entry with Some, None then we use end to add extra time to dur
        let mut found = false;
        let end = with.unwrap();
        for e in &self.entries {
            if e.start.is_some() && e.end.is_none() && e.entry_type == etype {
                debug_assert!(e.start.unwrap() <= end);
                if !found {
                    dur = dur + end.signed_duration_since(e.start.unwrap());
                    found = true;
                } else {
                    println!(
                        "WARNING: More than one entry with undefined end at: {}",
                        self.date
                    );
                }
            }
        }
        Ok(dur)
    }

    pub fn loggable_time(&self, _etype: TimeLogEntryType) -> Duration {
        if is_workday(self.date) {
            Duration::hours(8)
        } else {
            Duration::hours(0)
        }
    }

    pub fn logged_time(&self, etype: TimeLogEntryType) -> Duration {
        debug_assert!(self.validate_ordering());
        let mut sum = Duration::seconds(0);
        for e in &self.entries {
            if e.entry_type == etype {
                if etype == TimeLogEntryType::Work {
                    if let (Some(start), Some(end)) = (e.start, e.end) {
                        debug_assert!(e.start < e.end);
                        sum = sum + end.signed_duration_since(start);
                    }
                } else {
                    sum = sum + Duration::hours(8);
                }
            }
        }

        sum
    }

    pub fn has_unfinished_entries(&self) -> bool {
        self.entries.iter().any(|&e| {
            e.entry_type == TimeLogEntryType::Work && (e.start.is_none() || e.end.is_none())
                || (e.start.is_some() && e.end.is_none() || e.start.is_none() && e.end.is_some())
        })
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
        let mut v: Vec<TimeLogEntry> = Vec::new();
        for l in s.lines() {
            v.push(l.trim().parse()?);
        }
        debug_assert!(!v.is_empty());
        debug_assert!(v.iter().all(|x| x.date == v[0].date));

        Ok(TimeLogDay {
            date: v[0].date,
            entries: v,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use chrono::NaiveTime;
    #[test]
    fn timelogentry_basic_mutators() {
        let tdy = Local::today().naive_local();
        let start_time = NaiveTime::from_hms(11, 30, 0);
        let mut entry = TimeLogEntry::start(tdy, TimeLogEntryType::Work, start_time);
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
        let all_undef_pl: TimeLogEntry = "2017/12/22 Fri | ParentalLeave UNDEF UNDEF"
            .parse()
            .unwrap();
        assert_eq!(all_undef_pl.entry_type, TimeLogEntryType::ParentalLeave);
        assert_eq!(all_undef_pl.date, NaiveDate::from_ymd(2017, 12, 22));
        let all_undef_s: TimeLogEntry = "2017/12/22 Fri | Sickness UNDEF UNDEF".parse().unwrap();
        assert_eq!(all_undef_s.entry_type, TimeLogEntryType::Sickness);
        assert_eq!(all_undef_s.date, NaiveDate::from_ymd(2017, 12, 22));
    }

    #[test]
    fn timelogentry_consistent_serialiation() {
        let all_undef = "2017/12/22 Fri | Work UNDEF UNDEF";
        assert_eq!(
            all_undef,
            all_undef.parse::<TimeLogEntry>().unwrap().to_string()
        );

        let start = "2017/12/22 Fri | Work 07:31:00 UNDEF";
        assert_eq!(start, start.parse::<TimeLogEntry>().unwrap().to_string());

        let start_end = "2017/12/22 Fri | Work 07:31:00 12:00:00";
        assert_eq!(
            start_end,
            start_end.parse::<TimeLogEntry>().unwrap().to_string()
        );

        let all_undef_vac = "2017/12/22 Fri | Vacation UNDEF UNDEF";
        assert_eq!(
            all_undef_vac,
            all_undef_vac.parse::<TimeLogEntry>().unwrap().to_string()
        );
        let all_undef_pl = "2017/12/22 Fri | ParentalLeave UNDEF UNDEF";
        assert_eq!(
            all_undef_pl,
            all_undef_pl.parse::<TimeLogEntry>().unwrap().to_string()
        );
        let all_undef_s = "2017/12/22 Fri | Sickness UNDEF UNDEF";
        assert_eq!(
            all_undef_s,
            all_undef_s.parse::<TimeLogEntry>().unwrap().to_string()
        );
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
        let s = format!(
            "{}\n{}\n{}\n{}\n{}",
            all_undef, start, all_undef_vac, all_undef_pl, all_undef_s
        );

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
        let s = format!(
            "{}\n{}\n{}\n{}\n{}",
            all_undef, start, all_undef_vac, all_undef_pl, all_undef_s
        );

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
            "2017/12/18 Mon | Work UNDEF UNDEF\n",
        ];
        let mut s = String::new();
        for e in entries {
            s.push_str(e);
        }

        let mut day: TimeLogDay = s.as_str().parse().unwrap();

        day.set_start(NaiveTime::from_hms(06, 30, 00), TimeLogEntryType::Work);
        assert_eq!(day.entries[0].start, Some(NaiveTime::from_hms(6, 30, 0)));
        assert_eq!(day.entries[0].end, Some(NaiveTime::from_hms(7, 0, 0)));
        assert_eq!(day.entries[0].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[0].date, NaiveDate::from_ymd(2017, 12, 18));

        day.set_start(NaiveTime::from_hms(12, 30, 00), TimeLogEntryType::Work);
        assert_eq!(day.entries[2].start, Some(NaiveTime::from_hms(12, 30, 0)));
        assert_eq!(day.entries[2].end, None);
        assert_eq!(day.entries[2].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[2].date, NaiveDate::from_ymd(2017, 12, 18));

        day.set_end(NaiveTime::from_hms(12, 25, 00), TimeLogEntryType::Work);
        assert_eq!(day.entries[1].start, Some(NaiveTime::from_hms(7, 31, 0)));
        assert_eq!(day.entries[1].end, Some(NaiveTime::from_hms(12, 25, 0)));
        assert_eq!(day.entries[1].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[1].date, NaiveDate::from_ymd(2017, 12, 18));

        day.set_end(NaiveTime::from_hms(19, 12, 00), TimeLogEntryType::Work);
        assert_eq!(day.entries[2].start, Some(NaiveTime::from_hms(12, 30, 0)));
        assert_eq!(day.entries[2].end, Some(NaiveTime::from_hms(19, 12, 0)));
        assert_eq!(day.entries[2].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[2].date, NaiveDate::from_ymd(2017, 12, 18));
    }

    #[test]
    fn timelogday_diff_entry_types() {
        let entries = vec![
            "2017/12/18 Mon | Work UNDEF 07:00:00\n",
            "2017/12/18 Mon | Sickness 07:31:00 UNDEF",
        ];
        let mut s = String::new();
        for e in entries {
            s.push_str(e);
        }

        let mut day: TimeLogDay = s.as_str().parse().unwrap();

        day.set_start(NaiveTime::from_hms(06, 30, 00), TimeLogEntryType::Work);
        assert_eq!(day.entries[0].start, Some(NaiveTime::from_hms(6, 30, 0)));
        assert_eq!(day.entries[0].end, Some(NaiveTime::from_hms(7, 0, 0)));
        assert_eq!(day.entries[0].entry_type, TimeLogEntryType::Work);
        assert_eq!(day.entries[0].date, NaiveDate::from_ymd(2017, 12, 18));

        day.set_start(NaiveTime::from_hms(12, 30, 00), TimeLogEntryType::Sickness);
        assert_eq!(day.entries[2].start, Some(NaiveTime::from_hms(12, 30, 0)));
        assert_eq!(day.entries[2].end, None);
        assert_eq!(day.entries[2].entry_type, TimeLogEntryType::Sickness);
        assert_eq!(day.entries[2].date, NaiveDate::from_ymd(2017, 12, 18));

        day.set_end(NaiveTime::from_hms(12, 25, 00), TimeLogEntryType::Sickness);
        assert_eq!(day.entries[1].start, Some(NaiveTime::from_hms(7, 31, 0)));
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

    #[test]
    fn timelogday_time_logged_with() {
        let entries = vec![
            "2017/12/18 Mon | Work 06:00:00 07:00:00\n",
            "2017/12/18 Mon | Work 07:31:00 UNDEF\n",
        ];
        let mut s = String::new();
        for e in entries {
            s.push_str(e);
        }

        let day: TimeLogDay = s.as_str().parse().unwrap();
        let etype = TimeLogEntryType::Work;
        assert_eq!(
            day.time_logged_with(Some(NaiveTime::from_hms(8, 0, 0)), etype),
            Ok(Duration::minutes(89))
        );
    }
}
