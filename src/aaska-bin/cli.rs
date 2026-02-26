use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "aaska")]
#[command(about = "A static site generator", long_about = None)]
pub struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Increase verbosity level (-v for Info, -vv for Debug, -vvv for Trace)
    #[arg(short = 'v',  action = clap::ArgAction::Count)]
    verbosity: u8,
}

impl Args {
    pub fn command(&self) -> Command {
        self.command.clone().unwrap_or(Command::Run { path: None })
    }
    pub fn log_level(&self) -> tracing::Level {
        match self.verbosity {
            0 => tracing::Level::WARN,
            1 => tracing::Level::INFO,
            2 => tracing::Level::DEBUG,
            _ => tracing::Level::TRACE,
        }
    }
}

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    #[command(about = "Run the Aaska development server (default)")]
    Run {
        #[arg(short, long, help = "Path to the config file (optional)")]
        path: Option<String>,
    },
}

pub fn parse_args() -> Args {
    Args::parse()
}
