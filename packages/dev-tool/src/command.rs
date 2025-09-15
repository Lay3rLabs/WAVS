pub mod deploy_service;
pub mod send_triggers;

use clap::Parser;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub enum Command {
    DeployService,
    SendTriggers {
        #[arg(short, long, default_value_t = 1)]
        count: usize,
    },
}
