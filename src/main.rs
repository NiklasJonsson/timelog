mod commands;
mod timelog;
mod timelogger;

use crate::timelogger::TimeLogger;

use cli::clap;

const _USAGE: &str = "
Timelog

A commandline utility to log time. It maintains a human-editable time log
in ~/.timelog.

Usage:
  timelog start [<time>]
  timelog end [<time>]
  timelog month [--with <time>]
  timelog week [--with <time>]
  timelog week [--last]
  timelog day [--with <time>]
  timelog day [--last]
  timelog day [--mon | --tue | --wed | --thu | --fri]
  timelog batch --from <from> --to <to> --type <type> [--weekday-only]
  timelog view <n-entries>
  timelog (-h | --help)

Options:
  -h, --help                Show this screen.
  -w, --with <time>         If there is no end time for an entry, this will be used instead.
";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let log_location = dirs::home_dir().expect("No home dir?").push("timelog.txt");
    let log_location = "./timelog.txt";
    let clap = clap::Command::new("Timelog").about(format!(
        "A commandline utility to log time with a human readable log format.
        
The log is located at {log_location}"
    ));
    let mut cli = cli::Cli::from_clap(clap);
    cli.register_legacy("start", commands::start);
    cli.register_legacy("end", commands::end);
    cli.register_legacy("month", commands::month);
    cli.register_legacy("week", commands::week);
    cli.register_legacy("day", commands::day);
    cli.register_legacy("view", commands::view);
    cli.register_legacy("batch", commands::batch);

    let tl = match TimeLogger::at_path(log_location) {
        Ok(x) => x,
        Err(e) => {
            return Err(format!("ERROR: Could not create Timelogger instance: {}", e).into());
        }
    };

    let mut ctx = cli::Globals::new();
    ctx.insert(tl);
    cli.exec_legacy(std::env::args(), &mut ctx)
        .map_err(|e| e.into())
}
