pub use cli_derive::command;

use std::any::Any;

pub use clap;

pub struct Context {
    args: Vec<String>,
    global: Option<Box<dyn Any>>,
}

#[derive(Debug)]
pub enum Error {
    NoSuchCommand { args: Vec<String> },
    MissingArg { idx: usize },
    ArgParse(ArgParseError),
    IO(std::io::Error),
    App(String),
}

impl From<ArgFetchError> for Error {
    fn from(value: ArgFetchError) -> Self {
        match value {
            ArgFetchError::MissingArg { idx } => Self::MissingArg { idx },
            ArgFetchError::Parse(err) => Self::ArgParse(err),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IO(value)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Improve this
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error {}

pub type Result = std::result::Result<(), Error>;

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    pub fn output(&self) -> impl std::io::Write + 'static {
        std::io::stdout()
    }

    pub fn error(&self) -> impl std::io::Write + 'static {
        std::io::stderr()
    }

    pub fn arg(&self, idx: usize) -> Option<&str> {
        self.args.get(idx).map(String::as_str)
    }

    pub fn new() -> Self {
        Self {
            args: Vec::new(),
            global: None,
        }
    }

    pub fn global<T>(&mut self) -> Option<&mut T>
    where
        T: 'static,
    {
        self.global.as_mut().and_then(|x| x.downcast_mut::<T>())
    }

    pub fn set_global<T>(&mut self, global: T) -> Option<Box<dyn Any>>
    where
        T: 'static,
    {
        self.global.replace(Box::new(global))
    }
}

pub trait CommandLegacy {
    fn exec(&self, ctx: &mut Context) -> Result;
}

pub trait Command {
    fn exec(&self, ctx: &mut Context, args: &clap::ArgMatches) -> Result;
}

struct CommandEntryLegacy {
    name: String,
    args: Vec<String>,
    cmd: Box<dyn CommandLegacy>,
}

struct CommandEntry {
    name: String,
    cmd: Box<dyn Command>,
}

pub struct Cli {
    commands_legacy: Vec<CommandEntryLegacy>,
    commands: Vec<CommandEntry>,
    clap_builder: clap::Command,
}

impl Default for Cli {
    fn default() -> Self {
        Self::new()
    }
}

impl Cli {
    pub fn new() -> Self {
        Self {
            commands_legacy: Vec::new(),
            commands: Vec::new(),
            clap_builder: clap::Command::new("FIXME"),
        }
    }

    pub fn from_clap(cmd: clap::Command) -> Self {
        Self {
            commands_legacy: Vec::new(),
            commands: Vec::new(),
            clap_builder: cmd,
        }
    }
}

pub trait IntoCommand<A> {
    fn create(self) -> Box<dyn CommandLegacy>;
    fn args(&self) -> Vec<String>;
}

#[derive(Debug)]
pub struct ArgParseError;

pub trait Arg: Sized {
    fn parse(input: &str) -> std::result::Result<Self, ArgParseError>;
}

pub enum ArgFetchError {
    Parse(ArgParseError),
    MissingArg { idx: usize },
}

impl From<ArgParseError> for ArgFetchError {
    fn from(value: ArgParseError) -> Self {
        Self::Parse(value)
    }
}

pub trait ArgFetch: Sized {
    fn fetch(ctx: &Context, idx: usize) -> std::result::Result<Self, ArgFetchError>;
}

impl<T> ArgFetch for T
where
    T: Arg,
{
    fn fetch(ctx: &Context, idx: usize) -> std::result::Result<Self, ArgFetchError> {
        let input = ctx.arg(idx).ok_or(ArgFetchError::MissingArg { idx })?;
        <T as Arg>::parse(input).map_err(ArgParseError::into)
    }
}

impl<T> ArgFetch for Option<T>
where
    T: Arg,
{
    fn fetch(ctx: &Context, idx: usize) -> std::result::Result<Self, ArgFetchError> {
        match ctx.arg(idx) {
            Some(arg) => Ok(Some(<T as Arg>::parse(arg)?)),
            None => Ok(None),
        }
    }
}

impl Arg for String {
    fn parse(input: &str) -> std::result::Result<Self, ArgParseError> {
        Ok(input.to_owned())
    }
}

impl Arg for bool {
    fn parse(input: &str) -> std::result::Result<Self, ArgParseError> {
        Ok(!input.is_empty())
    }
}

impl Arg for usize {
    fn parse(input: &str) -> std::result::Result<Self, ArgParseError> {
        input.parse::<usize>().map_err(|_e| ArgParseError)
    }
}

impl<F> IntoCommand<()> for F
where
    F: Fn(&mut Context) -> Result + 'static,
{
    fn create(self) -> Box<dyn CommandLegacy> {
        Box::new(FnCommandLegacy {
            fun: Box::new(move |ctx| (self)(ctx)),
        })
    }

    fn args(&self) -> Vec<String> {
        Vec::new()
    }
}

impl<F, A> IntoCommand<A> for F
where
    A: ArgFetch + 'static,
    Self: Fn(&mut Context, A) -> Result + 'static,
{
    fn create(self) -> Box<dyn CommandLegacy> {
        Box::new(FnCommandLegacy {
            fun: Box::new(move |ctx| (self)(ctx, <A as ArgFetch>::fetch(ctx, 0)?)),
        })
    }

    fn args(&self) -> Vec<String> {
        vec![String::from("0")]
    }
}

impl<F, A0, A1> IntoCommand<(A0, A1)> for F
where
    A0: ArgFetch + 'static,
    A1: ArgFetch + 'static,
    Self: Fn(&mut Context, A0, A1) -> Result + 'static,
{
    fn create(self) -> Box<dyn CommandLegacy> {
        Box::new(FnCommandLegacy {
            fun: Box::new(move |ctx| {
                (self)(
                    ctx,
                    <A0 as ArgFetch>::fetch(ctx, 0)?,
                    <A1 as ArgFetch>::fetch(ctx, 1)?,
                )
            }),
        })
    }

    fn args(&self) -> Vec<String> {
        vec![String::from("0"), String::from("1")]
    }
}

impl<F, A0, A1, A2> IntoCommand<(A0, A1, A2)> for F
where
    A0: ArgFetch + 'static,
    A1: ArgFetch + 'static,
    A2: ArgFetch + 'static,
    Self: Fn(&mut Context, A0, A1, A2) -> Result + 'static,
{
    fn create(self) -> Box<dyn CommandLegacy> {
        Box::new(FnCommandLegacy {
            fun: Box::new(move |ctx| {
                (self)(
                    ctx,
                    <A0 as ArgFetch>::fetch(ctx, 0)?,
                    <A1 as ArgFetch>::fetch(ctx, 1)?,
                    <A2 as ArgFetch>::fetch(ctx, 2)?,
                )
            }),
        })
    }

    fn args(&self) -> Vec<String> {
        vec![String::from("0"), String::from("1"), String::from("2")]
    }
}

impl<F, A0, A1, A2, A3> IntoCommand<(A0, A1, A2, A3)> for F
where
    A0: ArgFetch + 'static,
    A1: ArgFetch + 'static,
    A2: ArgFetch + 'static,
    A3: ArgFetch + 'static,
    Self: Fn(&mut Context, A0, A1, A2, A3) -> Result + 'static,
{
    fn create(self) -> Box<dyn CommandLegacy> {
        Box::new(FnCommandLegacy {
            fun: Box::new(move |ctx| {
                (self)(
                    ctx,
                    <A0 as ArgFetch>::fetch(ctx, 0)?,
                    <A1 as ArgFetch>::fetch(ctx, 1)?,
                    <A2 as ArgFetch>::fetch(ctx, 2)?,
                    <A3 as ArgFetch>::fetch(ctx, 3)?,
                )
            }),
        })
    }

    fn args(&self) -> Vec<String> {
        vec![
            String::from("0"),
            String::from("1"),
            String::from("2"),
            String::from("3"),
        ]
    }
}

struct FnCommandLegacy {
    fun: Box<dyn Fn(&mut Context) -> Result>,
}

struct FnCommand {
    fun: Box<dyn Fn(&mut Context, &::clap::ArgMatches) -> Result>,
}

impl Command for FnCommand {
    fn exec(&self, ctx: &mut Context, args: &clap::ArgMatches) -> Result {
        (self.fun)(ctx, args)
    }
}

impl CommandLegacy for FnCommandLegacy {
    fn exec(&self, ctx: &mut Context) -> Result {
        (self.fun)(ctx)
    }
}

impl Cli {
    pub fn register_legacy<S, Args>(&mut self, name: S, cmd: impl IntoCommand<Args>)
    where
        S: Into<String>,
    {
        self.commands_legacy.push(CommandEntryLegacy {
            name: name.into(),
            args: cmd.args(),
            cmd: cmd.create(),
        });
    }

    pub fn exec_legacy(&mut self, args: std::env::Args, ctx: &mut Context) -> Result {
        todo!()
        /*
              let args = args.collect::<Vec<String>>();

              let mut cmd_builder: clap::Command = self.clap_builder.clone();

              for CommandEntryLegacy { name, args, .. } in &self.commands_legacy {
                  let mut subcmd = clap::Command::new(name);
                  for arg in args {
                      subcmd = subcmd.arg(clap::Arg::new(arg));
                  }
                  cmd_builder = cmd_builder.subcommand(subcmd);
              }

              let clap_cli = cmd_builder.get_matches_from(&args);

              for CommandEntryLegacy { name, cmd, .. } in &self.commands_legacy {
                  if let Some(_matches) = clap_cli.subcommand_matches(name) {
                      log::debug!("Executing {name} with args: {:?}", ctx.args);
                      return cmd.exec(ctx);
                  }
              }

              Err(Error::NoSuchCommand { args })
        */
    }
}

impl Cli {
    pub fn register(&mut self, register_cmd: impl FnOnce(&mut Cli)) {
        register_cmd(self);
    }

    pub fn register_command(
        &mut self,
        clap_cmd: clap::Command,
        fn_impl: impl Fn(&mut Context, &::clap::ArgMatches) -> Result + 'static,
    ) {
        let name = clap_cmd.get_name().to_string();
        self.clap_builder = self.clap_builder.clone().subcommand(clap_cmd);
        self.commands.push(CommandEntry {
            name,
            cmd: Box::new(FnCommand {
                fun: Box::new(fn_impl),
            }),
        });
    }

    pub fn exec(&mut self, args: std::env::Args, ctx: &mut Context) -> Result {
        let args = args.collect::<Vec<String>>();
        let cmd = self.clap_builder.clone();
        let matches = cmd.get_matches_from(&args);

        if let Some((subcmd, args)) = matches.subcommand() {
            for cand in &self.commands {
                if cand.name == subcmd {
                    return cand.cmd.exec(ctx, args);
                }
            }
        }

        Err(Error::NoSuchCommand { args })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test0(_: &mut Context) -> Result {
        Ok(())
    }

    fn test1(_: &mut Context, _: String) -> Result {
        Ok(())
    }

    fn test2(_: &mut Context, _: Option<String>) -> Result {
        Ok(())
    }

    #[test]
    fn test_register() {
        let mut cli = Cli::new();
        cli.register_legacy("test0", test0);
        cli.register_legacy("test1", test1);
        cli.register_legacy("test2", test2);
    }
}
