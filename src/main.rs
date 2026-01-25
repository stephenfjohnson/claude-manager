mod app;
mod db;
mod detect;
mod errors;
mod gh;
mod git_status;
mod machine;
mod ports;
mod process;
mod sync;
mod tui;
mod ui;

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
        run_init()?;
    } else {
        run_tui()?;
    }

    Ok(())
}

fn run_init() -> anyhow::Result<()> {
    println!("Initializing Claude Manager...\n");

    let machine_id = machine::get_or_create_machine_id()?;
    println!("Machine ID: {}", machine_id);

    sync::init()?;
    println!("Sync repo ready.");

    let db_path = sync::db_path()?;
    let _db = db::Database::open(&db_path)?;
    println!("Database initialized.");

    println!("\nInitialization complete! Run 'claude-manager' to start.");
    Ok(())
}

fn run_tui() -> anyhow::Result<()> {
    if !sync::is_initialized()? {
        return Err(AppError::NotInitialized.into());
    }

    sync::pull()?;

    let machine_id = machine::get_machine_id()?.ok_or(AppError::NotInitialized)?;
    let db_path = sync::db_path()?;
    let db = db::Database::open(&db_path)?;

    let mut app = app::App::new(db, machine_id)?;
    let mut tui = tui::Tui::new()?;
    tui.run(&mut app)?;

    Ok(())
}
