extern crate chrono;
extern crate regex;

use std::path::PathBuf;
use std::path::Path;
use std::io::BufReader;
use std::io::prelude::*;
use std::fs::File;
use std::env::*;
use std;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::str::FromStr;

use self::regex::Regex;

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
const month2ndays: [usize; MONTHS_IN_YEAR] = [31,28,31,30,31,30,31,31,30,31,30,31];
const month2str: [&str; MONTHS_IN_YEAR] =
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

#[derive(PartialEq, Copy, Clone)]
enum Month {
    January = 0,
    February = 1,
    March = 2,
    April = 3,
    May = 4,
    June = 5,
    July = 6,
    August = 7,
    September = 8,
    October = 9,
    November = 10,
    December = 11,
}

impl Month {
    fn from_u32(n: u32) -> Month {
        match n {
            0 => Month::January,
            1 => Month::February,
            2 => Month::March,
            3 => Month::April,
            4 => Month::May,
            5 => Month::June,
            6 => Month::July,
            7 => Month::August,
            8 => Month::September,
            9 => Month::October,
            10 => Month::November,
            11 => Month::December,
            _ => panic!("Invalid month number"),
        }
    }
}


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
    day: Weekday,
    day_idx: usize,
    month_idx: usize,
}

const DAY_INDENT: &str = "  ";
impl TimeLogDay {
    fn new(day: Weekday, day_idx: usize, month_idx: usize) -> Self {
        TimeLogDay{day: day, start: None, end: None, acc_break: Duration::seconds(0), day_idx: day_idx, month_idx: month_idx}
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
        return self.day != Weekday::Sat && self.day != Weekday::Sun;
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

/*
 * <DD/MM> <Weekday>
 *   Start: <NaiveTime>
 *   End: <NaiveTime>
 *   Accumulated break: <Duration>
 */
impl FromStr for TimeLogDay {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lines: Vec<&str> = s.lines().collect();
        let date_re = Regex::new(r"(\d\d)/(\d\d) (\w+)").unwrap();
        let caps = date_re.captures(lines[0]).ok_or("Invalid file contents")?;
        let day_idx = usize::from_str(&caps[1]).unwrap();
        let month_idx = usize::from_str(&caps[2]).unwrap();

        let wday: Weekday = Weekday::from_str(caps[3].trim()).unwrap();
        let start = try_get_naivetime(lines[1].split(' ').nth(1).unwrap().trim());
        let end = try_get_naivetime(lines[2].split(' ').nth(1).unwrap().trim());
        let acc_br = parse_duration(lines[3].split(' ').nth(2).unwrap().trim()).unwrap();
        return Ok(TimeLogDay{start: start, end:end, acc_break: acc_br,
            day: wday, day_idx: day_idx, month_idx: month_idx});
    }
}

impl Display for TimeLogDay {
// TODO: Remove everything but hours and minute for NaiveTime before printing

    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut s = String::new();
        s.push_str(&format!("{:02}/{:02} {:?}\n", self.day_idx, self.month_idx, self.day));
        if let Some(x) = self.start {
            s.push_str(format!("{}Start: {}\n", DAY_INDENT, x).as_str());
        } else {
            s.push_str("  Start: UNDEF\n");
        }
        if let Some(x) = self.end {
            s.push_str(format!("{}Start: {}\n", DAY_INDENT, x).as_str());
        } else {
            s.push_str("  End: UNDEF\n");
        }
        s.push_str(&format!("{}Accumulated break: {:02};{:02}\n", DAY_INDENT, self.acc_break.num_hours(), self.acc_break.num_minutes() % 60));
        write!(f, "{}", s)
    }
}

struct TimeLogMonth {
    month: Month,
    n_days: usize,
    days: Vec<TimeLogDay>,
}

impl TimeLogMonth {
    fn empty(month: Month, first_weekday: Weekday) -> Self {
        let n_days = month2ndays[month as usize];
        debug_assert!(n_days <= MAX_DAYS_IN_MONTH, "Number of days in month is too large");
        let mut days = Vec::with_capacity(n_days);
        let mut wd = first_weekday;
        for i in 0..n_days {
            days.push(TimeLogDay::new(wd, i + 1, month as usize));
            wd = wd.succ();
        }
        TimeLogMonth{n_days: n_days, month: month, days: days}
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

fn str2month(s: &str) -> Month {
    match s {
        "January" => Month::January,
        "February" => Month::February,
        "March" => Month::March,
        "April" => Month::April,
        "May" => Month::May,
        "June" => Month::June,
        "July" => Month::July,
        "August" => Month::August,
        "September" => Month::September,
        "October" => Month::October,
        "November" => Month::November,
        "December" => Month::December,
        _ => panic!("Can't find month in: {}", s),
    }
}

impl FromStr for TimeLogMonth {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut days: Vec<TimeLogDay> = Vec::with_capacity(MAX_DAYS_IN_MONTH);
        let mut line_it = s.lines();

        let month: Month = str2month(match line_it.next() {
            None => return Err("Can't read first line from logfile".to_owned()),
            Some(x) => x,
        });


        let days_it = line_it.enumerate().fold(Vec::new(), |mut acc: Vec<String>, (i, x)| {
            if i % 4 == 0 {
                acc.push(String::new());
            }
            acc[i / 4].push_str(x.trim());
            acc[i / 4].push('\n');
            return acc;
        });

        let mut i = 0;
        for day in days_it {
            days.push(TimeLogDay::from_str(day.as_str()).unwrap());
            i += 1;
        }

        Ok(TimeLogMonth{month: month, n_days: month2ndays[month as usize], days: days})
    }
}

impl fmt::Display for TimeLogMonth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::new();
        s.push_str(month2str[self.month as usize]);
        s.push_str("\n");
        for i in 0..self.n_days {
            s.push_str(self.days[i].to_string().as_str());
        }
        write!(f, "{}", s)
    }
}

pub struct TimeLogger {
    tl_month: TimeLogMonth,
    today_idx: usize,
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
        let month = Local::today().month0();
        let mut path_buf = std::env::home_dir().unwrap();
        path_buf.push(TIMELOGGER_FOLDER);

        if !path_buf.as_path().exists() {
            std::fs::create_dir(path_buf.as_path());
        }

        path_buf.push(year.to_string());

        if !path_buf.as_path().exists() {
            std::fs::create_dir(path_buf.as_path());
        }

        path_buf.push(month2str[month as usize]);
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
                TimeLogMonth::empty(Month::from_u32(month), NaiveDate::from_ymd(year, month, 1).weekday())
            },
        };

        TimeLogger{today_idx: Local::today().day0() as usize, tl_month: tlm, file_path: path_buf}
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
