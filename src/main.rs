mod errors;

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

    if cli.init {
        println!("Init mode - not yet implemented");
    } else {
        println!("TUI mode - not yet implemented");
    }

    Ok(())
}
