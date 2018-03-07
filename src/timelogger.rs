extern crate chrono;
use timelog::*;

use std::path::PathBuf;
use std::path::Path;
use std::io::BufReader;
use std::io::prelude::*;
use std::fs::File;
use std;
use std::fs;
use std::collections::HashMap;
use std::collections::hash_map::Entry::{Occupied, Vacant};

use chrono::NaiveTime;
use chrono::Duration;
use chrono::Weekday;
use chrono::prelude::*;

const MONTH_2_NDAYS: [u32; 12] = [31,28,31,30,31,30,31,31,30,31,30,31];

fn get_monday_in_week_of(date: NaiveDate) -> NaiveDate {
    let mut first_day = date;
    while first_day.weekday() != Weekday::Mon {
        first_day = first_day.pred();
    }

    return first_day;
}

fn get_friday_in_week_of(date: NaiveDate) -> NaiveDate {
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

macro_rules! gen_time_logged_in_timeperiod_with {
    ($fname: ident, $start_date_f: ident, $end_date_f: ident) => {
        pub fn $fname(&self, date: NaiveDate, with: Option<NaiveTime>) -> TimeLogResult<Duration> {
            let start_date = $start_date_f(date);
            let end_date = $end_date_f(date);
            let logged_time = TimeLogEntryType::iterator()
                .map(|x| self.compute_logged_time_between(start_date, end_date, *x))
                .fold(Duration::hours(0), |acc, e| acc + e);

            match with {
                None => { return Ok(logged_time); },
                Some(_) => {
                    let mut last_date_with_entries = start_date;
                    while self.date2logday.get(&last_date_with_entries.succ()).is_some() && last_date_with_entries != end_date {
                        last_date_with_entries = last_date_with_entries.succ();
                    }

                    let work_et = TimeLogEntryType::Work;
                    let ref last_tld = self.date2logday[&last_date_with_entries];
                    let last_tld_logged = last_tld.logged_time(work_et);
                    let last_tld_logged_with = last_tld.time_logged_with(with, work_et)?;
                    return Ok(logged_time - last_tld_logged + last_tld_logged_with);
                }
            }
        }
    }
}

macro_rules! gen_time_left_in_timeperiod_with {
    ($fname: ident, $loggable_f: ident, $logged_f: ident) => {
        pub fn $fname(&self, date: NaiveDate, with: Option<NaiveTime>) -> TimeLogResult<(Duration, Duration)> {
            let etype = TimeLogEntryType::Work;
            let workable_time = self.$loggable_f(date, etype);
            let logged_time = self.$logged_f(date, with)?;
            let flex_time = self.flextime_as_of(date);
            return Ok((workable_time - logged_time + flex_time, flex_time));
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
            s.push('\n');
        }

        return s;
    }

    fn read_entries(&mut self, s: &str) -> TimeLogResult<()> {
        let mut lines = s.lines();
        while let Some(line) = lines.next() {
            let tle: TimeLogEntry = line.parse()?;
            let date = tle.get_date();

            let tld = match self.date2logday.entry(date) {
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
    gen_x_in_y_of!(compute_loggable_time_in_week_of, compute_loggable_time_between,
                   get_monday_in_week_of, get_friday_in_week_of);

    gen_log!(log_start, set_start);
    gen_log!(log_end, set_end);

    fn flextime_as_of(&self, date: NaiveDate) -> Duration {
        let mut keys: Vec<&NaiveDate> = self.date2logday.keys().collect();
        keys.sort();

        if keys.len() == 0 {
            return Duration::hours(0);
        }

        let mut sunday_last_week = match date.weekday() {
            Weekday::Sun =>  date.pred(),
            _ =>  date,
        };

        while sunday_last_week.weekday() != Weekday::Sun {
            sunday_last_week = sunday_last_week.pred();
        }

        debug_assert!(sunday_last_week > *keys[0]);

        let start_date = *keys[0];
        let end_date = sunday_last_week;
        let logged_time = TimeLogEntryType::iterator()
            .map(|x| self.compute_logged_time_between(start_date, end_date, *x))
            .fold(Duration::hours(0), |acc, e| acc + e);

        return self.compute_loggable_time_between(start_date, end_date, TimeLogEntryType::Work) - logged_time;
    }

    pub fn time_logged_at_date_with(&self, date: NaiveDate, with: Option<NaiveTime>) -> TimeLogResult<Duration> {
        let etype = TimeLogEntryType::Work;
        let tld = self
            .date2logday
            .get(&date)
            .ok_or_else(|| TimeLogError::inv_inp(
                    format!("Can't find start time, no entries for date: {}\n", date).as_str()))?;


        return tld.time_logged_with(with, etype);
    }

    gen_time_logged_in_timeperiod_with!(time_logged_in_week_of_with, get_monday_in_week_of, get_friday_in_week_of);
    gen_time_logged_in_timeperiod_with!(time_logged_in_month_of_with, get_first_day_in_month_of, get_last_day_in_month_of);

    gen_time_left_in_timeperiod_with!(time_left_in_week_of_with, compute_loggable_time_in_week_of, time_logged_in_week_of_with);
    gen_time_left_in_timeperiod_with!(time_left_in_month_of_with, compute_loggable_time_in_month_of, time_logged_in_month_of_with);

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

    pub fn get_latest_n_entries(&self, n: usize) -> Vec<&TimeLogDay> {
        let mut days = Vec::with_capacity(n);
        let mut keys: Vec<&NaiveDate> = self.date2logday.keys().collect();
        keys.sort();

        for (i,k) in keys.iter().rev().enumerate() {
            if i >= n {
                break;
            }
            days.push(&self.date2logday[k]);
        }

        days.reverse();

        return days;
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;
    use chrono::Duration;

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

        let mon_tld = format!("{}\n{}", mon_1, mon_2).as_str().parse::<TimeLogDay>().unwrap();
        let tue_tld = format!("{}\n{}", tue_1, tue_2).as_str().parse::<TimeLogDay>().unwrap();
        let wed_tld = format!("{}\n{}", wed_1, wed_2).as_str().parse::<TimeLogDay>().unwrap();

        assert_eq!(logger.date2logday.len(), 3);
        assert_eq!(logger.date2logday[mon], mon_tld);
        assert_eq!(logger.date2logday[tue], tue_tld);
        assert_eq!(logger.date2logday[wed], wed_tld);

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

        let d40hr = Duration::hours(40);

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
    }

    #[test]
    fn timelogger_log_start_end() {
        let mut logger = TimeLogger{file_path: PathBuf::new(), date2logday: HashMap::new()};
        let today = NaiveDate::from_ymd(2018,01,01);
        let start = NaiveTime::from_hms(12,0,0);
        let end = NaiveTime::from_hms(13,0,0);
        logger.log_start(today, start);
        logger.log_end(today, end);
        let tld = "2018/01/01 Mon | Work 12:00:00 13:00:00".parse::<TimeLogDay>().unwrap();
        assert_eq!(logger.date2logday[&today], tld);
    }

    #[test]
    fn timelogger_flex_time() {
        let days = ["2017/12/11 Mon | Work 08:00:00 16:00:00\n", // 8
            "2017/12/12 Tue | Work 09:00:00 16:00:00\n", // 7
            "2017/12/13 Wed | Work 08:00:00 16:00:00\n", // 8
            "2017/12/14 Thu | Work 10:00:00 17:00:00\n", // 7
            "2017/12/15 Fri | Work 08:00:00 15:35:00\n", // 7;35
            // => 37;35
            "2017/12/18 Mon | Work 08:00:00 18:00:00\n", // 10
            "2017/12/19 Tue | Work 10:00:00 18:25:00\n", // 8;25
            "2017/12/20 Wed | Work 09:00:00 16:00:00\n", // 7
            "2017/12/21 Thu | Work 10:00:00 17:00:00\n", // 7
            "2017/12/22 Fri | Work 07:00:00 18:00:00\n", // 11
            // => 43;25
            "2017/12/25 Mon | Work 08:00:00 16:00:00\n", // 8
            "2017/12/26 Tue | Work 09:00:00 16:00:00\n", // 7
            "2017/12/27 Wed | Work 08:00:00 16:00:00\n", // 8
            "2017/12/28 Thu | Work 10:00:00 18:00:00\n", // 8
            "2017/12/29 Fri | Work 08:00:00 16:00:00\n", // 8
            // => 39
            "2018/01/01 Mon | Work 08:00:00 16:00:00\n"];

        let mut s = String::new();
        for d in days.into_iter() {
            s.push_str(d);
        }

        let mut logger = TimeLogger{file_path: PathBuf::new(), date2logday: HashMap::new()};
        logger.read_entries(s.as_str()).unwrap();

        let mon1 = NaiveDate::from_ymd(2017,12,18);
        let mon2 = NaiveDate::from_ymd(2017,12,25);
        let mon3 = NaiveDate::from_ymd(2018,01,01);
        let tue3 = NaiveDate::from_ymd(2018,01,02);

        assert_eq!(logger.flextime_as_of(mon1),  Duration::minutes(2*60 + 25));
        assert_eq!(logger.flextime_as_of(mon2),  -Duration::minutes(60));
        assert_eq!(logger.flextime_as_of(mon3),  Duration::minutes(0));
        assert_eq!(logger.flextime_as_of(tue3),  Duration::minutes(0));
    }

    #[test]
    fn timelogger_consistent_serialization() {
        let nov_mon_1 = "2017/11/13 Mon | Work 08:00:00 18:00:00";
        let nov_tue_1 = "2017/11/14 Tue | Work 07:30:00 12:00:00";
        let nov_wed_1 = "2017/11/15 Wed | Work 09:10:00 15:10:00";
        let mon_1 = "2017/12/18 Mon | Work 06:31:00 07:00:00";
        let mon_2 = "2017/12/18 Mon | Work 07:31:00 UNDEF";
        let tue_1 = "2017/12/19 Tue | Work 07:31:00 11:50:00";
        let tue_2 = "2017/12/19 Tue | Work 12:34:00 18:15:00";
        let wed_1 = "2017/12/20 Wed | Work 09:10:00 11:55:00";
        let wed_2 = "2017/12/20 Wed | Work 12:40:00 18:45:00";

        let s = format!("{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
                        nov_mon_1, nov_tue_1, nov_wed_1,
                        mon_1, mon_2, tue_1, tue_2, wed_1, wed_2);

        let mut logger = TimeLogger{file_path: PathBuf::new(), date2logday: HashMap::new()};
        logger.read_entries(s.as_str()).unwrap();
        assert_eq!(logger.write_entries(), s);
    }
}
