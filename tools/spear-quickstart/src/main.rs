use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod config;
mod deploy;
mod tui;

#[derive(Debug, Parser)]
#[command(name = "spear-quickstart")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Configure {
        #[arg(long)]
        config: PathBuf,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    Tui {
        #[arg(long)]
        config: PathBuf,
    },
    Plan {
        #[arg(long)]
        config: PathBuf,
    },
    Apply {
        #[arg(long)]
        config: PathBuf,
        #[arg(long, default_value_t = false)]
        yes: bool,
    },
    Status {
        #[arg(long)]
        config: PathBuf,
    },
    Cleanup {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long, default_value_t = false)]
        yes: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Command::Configure { config, force } => {
            config::ensure_default_config(&config, force)?;
            println!("{}", config.display());
        }
        Command::Tui { config } => {
            config::ensure_default_config(&config, false)?;
            let mut cfg = config::load_config(&config)?;
            tui::edit_config_tui(&mut cfg, &config)?;
            config::save_config(&config, &cfg)?;
            println!("{}", config.display());
        }
        Command::Plan { config } => {
            let cfg = config::load_config(&config)?;
            deploy::plan(&cfg)?;
        }
        Command::Apply { config, yes } => {
            let cfg = config::load_config(&config)?;
            deploy::apply(&cfg, yes)?;
        }
        Command::Status { config } => {
            let cfg = config::load_config(&config)?;
            deploy::status(&cfg)?;
        }
        Command::Cleanup { config, scope, yes } => {
            let cfg = config::load_config(&config)?;
            let scope = scope.unwrap_or_else(|| "release".to_string());
            deploy::cleanup(&cfg, &scope, yes)?;
        }
    }

    Ok(())
}
