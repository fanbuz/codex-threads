mod app_state;
pub mod cli;
pub mod commands;
mod experimental;
pub mod index;
pub mod output;
pub mod parser;
mod query;

use anyhow::Result;
use cli::Cli;
use output::Rendered;

pub fn run(cli: Cli) -> Result<Rendered> {
    commands::run(cli)
}
