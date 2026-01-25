mod db;
mod errors;
mod gh;
mod machine;
mod sync;

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

    // 1. Generate machine ID
    let machine_id = machine::get_or_create_machine_id()?;
    println!("Machine ID: {}", machine_id);

    // 2. Setup sync repo
    sync::init()?;
    println!("Sync repo ready.");

    // 3. Open database to ensure schema exists
    let db_path = sync::db_path()?;
    let _db = db::Database::open(&db_path)?;
    println!("Database initialized.");

    println!("\nInitialization complete! Run 'claude-manager' to start.");
    Ok(())
}

fn run_tui() -> anyhow::Result<()> {
    // Check if initialized
    if !sync::is_initialized()? {
        return Err(AppError::NotInitialized.into());
    }

    // Pull latest
    println!("Syncing...");
    sync::pull()?;

    // Load machine ID and database
    let machine_id = machine::get_machine_id()?.ok_or(AppError::NotInitialized)?;
    let db_path = sync::db_path()?;
    let db = db::Database::open(&db_path)?;

    let projects = db.list_projects()?;
    println!("\nProjects ({}):", projects.len());
    for p in &projects {
        let loc = db.get_location(p.id, &machine_id)?;
        let status = if loc.is_some() { "+" } else { "-" };
        println!("  {} {} ({})", status, p.name, p.repo_url);
    }

    println!("\nTUI not yet implemented. Press Ctrl+C to exit.");

    Ok(())
}
