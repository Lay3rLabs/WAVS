use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct CliArgs {
    /// The home directory of the application, where the wasmatic.toml configuration file is stored
    /// if not provided, a series of default directories will be tried
    #[arg(long)]
    pub home_dir: Option<PathBuf>,

    /// The path to an optional dotenv file to try and load
    /// if not set, will be the current working directory's .env
    #[arg(long)]
    pub dotenv: Option<PathBuf>,
}
