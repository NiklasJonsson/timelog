extern crate chrono;
extern crate regex;

use std::path::PathBuf;
use std::path::Path;
use std::io::BufReader;
use std::io::prelude::*;
use std::fs::File;
use std;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::str::FromStr;

use chrono::NaiveTime;
use chrono::Duration;
use chrono::Weekday;
use chrono::prelude::*;

pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let dur: Vec<&str> = s.split(';').map(|x| x.trim()).collect();
    let h: i64 = dur[0].parse().unwrap();
    let m: i64 = dur[1].parse().unwrap();

    return Ok(Duration::minutes(h * 60 + m));
}

const MAX_DAYS_IN_MONTH: usize = 31;
const MONTHS_IN_YEAR: usize = 12;
const MONTH_2_NDAYS: [usize; MONTHS_IN_YEAR] = [31,28,31,30,31,30,31,31,30,31,30,31];
const MONTH_2_STR: [&str; MONTHS_IN_YEAR] =
[
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/*
 * Format:
 * <month-name>
 * <DD0/MM> <Weekday>
 *   Start: <Time>
 *   End: <Time>
 *   Accumulated break: <Duration>
 * <DD1/MM> <weekday>
 *   Start: <Time>
 *   End: <Time>
 *   Accumulated break: <Duration>
 * Where DD is day with two numbers, MM is month.
 * <Time> may be a either be a HH:MM or UNDEF if the value has not been set yet
 * <Duration> will be HH;MM
 * <weekday> is Mon...Sun
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
        self.start = Some(time);
    }

    fn set_end(&mut self, time: NaiveTime) {
        self.end = Some(time);
    }

    fn add_break(&mut self, dur: Duration) {
        self.acc_break = self.acc_break + dur;
    }

    fn is_workday(&self) -> bool {
        return self.date.weekday() != Weekday::Sat && self.date.weekday() != Weekday::Sun;
    }
}

fn try_get_naivetime(s: &str) -> Option<NaiveTime> {
    if s.contains("UNDEF") {
        return None;
    } else {
        return match NaiveTime::from_str(s) {
            Err(_) => None,
            Ok(x) => Some(x),
        };
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
    type Err = chrono::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lines: Vec<&str> = s.lines().collect();
        let date = NaiveDate::parse_from_str(lines[0].trim(), TIMELOGDAY_NAIVEDATE_FORMAT_STRING!())?;

        let start = try_get_naivetime(lines[1].split(' ').nth(1).unwrap().trim());
        let end = try_get_naivetime(lines[2].split(' ').nth(1).unwrap().trim());
        let acc_br = parse_duration(lines[3].split(' ').nth(2).unwrap().trim()).unwrap();
        return Ok(TimeLogDay{start: start, end:end, acc_break: acc_br, date: date});
    }
}

impl Display for TimeLogDay {
// TODO: Remove everything but hours and minute for NaiveTime before printing

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

// FUTURE: Can we calculate n_days with NaiveDate instead fo function lookup?
struct TimeLogMonth {
    first_date: NaiveDate, // First date of the month. contains year month and day
    n_days: usize,
    days: Vec<TimeLogDay>,
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
        TimeLogMonth{n_days: n_days, days: days, first_date: first_date}
    }

    fn compute_hours_worked(&self) -> Duration {
        self.days.iter().fold(Duration::zero(), |acc, day| {
            if day.start.is_some() && day.end.is_some() {
                debug_assert!(day.end > day.start, "End of workday has to be after start");
                return acc + day.end.unwrap().signed_duration_since(day.start.unwrap()) - day.acc_break;
            }
            return acc;
        })
    }

    fn compute_workable_hours(&self) -> Duration {
        self.days.iter().fold(Duration::zero(), |acc, day| {
            if day.is_workday() {
                return acc + Duration::hours(8);
            }
            return acc;
        })
    }

    fn compute_hours_left(&self) -> Duration {
        self.compute_workable_hours() - self.compute_hours_worked()
    }
}

macro_rules! TIMELOGMONTH_NAIVEDATE_FORMAT_STRING {
    () => ("%B %Y");
}

impl FromStr for TimeLogMonth {
    type Err = chrono::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut days: Vec<TimeLogDay> = Vec::with_capacity(MAX_DAYS_IN_MONTH);
        let mut line_it = s.lines();

        let first_date = NaiveDate::parse_from_str(line_it.next().unwrap().trim(), TIMELOGMONTH_NAIVEDATE_FORMAT_STRING!())?;

        let days_it = line_it.enumerate().fold(Vec::new(), |mut acc: Vec<String>, (i, x)| {
            if i % 4 == 0 {
                acc.push(String::new());
            }
            acc[i / 4].push_str(x.trim());
            acc[i / 4].push('\n');
            return acc;
        });

        for day in days_it {
            days.push(TimeLogDay::from_str(day.as_str()).unwrap());
        }

        Ok(TimeLogMonth{first_date: first_date, n_days: MONTH_2_NDAYS[first_date.month0() as usize], days: days})
    }
}

impl fmt::Display for TimeLogMonth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::new();
        s.push_str(self.first_date.format(TIMELOGDAY_NAIVEDATE_FORMAT_STRING!()).to_string().as_str());
        s.push_str("\n");
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

    pub fn current_month() -> Self {
        /* Directory structure is:
         * $HOME/.timelog/
         *   2017/
         *      January.tl
         *      February.tl
         *   2018/
         */
        let year = Local::today().year();
        let month = Local::today().month();
        let mut path_buf = std::env::home_dir().unwrap();
        path_buf.push(TIMELOGGER_FOLDER);

        if !path_buf.as_path().exists() {
            std::fs::create_dir(path_buf.as_path());
        }

        path_buf.push(year.to_string());

        if !path_buf.as_path().exists() {
            std::fs::create_dir(path_buf.as_path());
        }

        path_buf.push(MONTH_2_STR[month as usize]);
        path_buf.set_extension("tl");

        if !path_buf.as_path().exists() {
            File::create(path_buf.as_path());
        }

        debug_assert!(path_buf.as_path().exists(), "logfile does not exist");

        let file = match File::open(path_buf.as_path()) {
            Err(_) => File::create(path_buf.as_path()).unwrap(),
            Ok(file) => file,
        };
        let mut buf_reader = BufReader::new(file);
        let mut contents = String::new();
        buf_reader.read_to_string(&mut contents);
        let tlm: TimeLogMonth = match contents.parse() {
            Ok(x) => x,
            Err(_) => {
                TimeLogMonth::empty(NaiveDate::from_ymd(year, month, 1))
            },
        };

        TimeLogger{tl_month: tlm, file_path: path_buf}
    }

    pub fn hours_left_this_week(&self) -> u32 {
        debug_assert!(false, "Not implemented");
        return 0;
    }

    pub fn hours_left_this_month(&self) -> u32 {
        self.tl_month.compute_hours_left().num_hours() as u32
    }

    pub fn total_hours_this_month(&self) -> u32 {
        self.tl_month.compute_workable_hours().num_hours() as u32
    }

    pub fn log_start(&mut self, time: NaiveTime) {
        self.tl_month.days[Local::now().day0() as usize].set_start(time);
    }

    pub fn log_end(&mut self, time: NaiveTime) {
        self.tl_month.days[Local::now().day0() as usize].set_end(time);
    }

    pub fn log_break(&mut self, dur: Duration) {
        self.tl_month.days[Local::now().day0() as usize].add_break(dur);
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let fp: &Path = self.file_path.as_path();
        debug_assert!(fp.exists(), "logfile does not exist");
        // TODO: Write to backup file and then write actual logfile
        let mut file = File::create(fp)?;
        let s = format!("{}", self.tl_month);
        file.write_all(s.as_str().as_bytes()).unwrap();
        Ok(())
    }
}
