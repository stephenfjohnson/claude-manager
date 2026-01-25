mod errors;
mod gh;
mod machine;

use crate::errors::AppError;
use clap::Parser;

#[derive(Parser)]
#[command(name = "claude-manager")]
#[command(about = "Personal project dashboard across machines")]
struct Cli {
    /// Initialize Claude Manager (run once per machine)
    #[arg(long)]
    init: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Always check gh auth first
    if !gh::check_auth()? {
        return Err(AppError::GhNotAuthenticated.into());
    }

    if cli.init {
        let machine_id = machine::get_or_create_machine_id()?;
        let username = gh::get_username()?;
        println!("Machine ID: {}", machine_id);
        println!("GitHub user: {}", username);
    } else {
        match machine::get_machine_id()? {
            Some(id) => println!("Machine ID: {}", id),
            None => return Err(AppError::NotInitialized.into()),
        }
    }

    Ok(())
}
