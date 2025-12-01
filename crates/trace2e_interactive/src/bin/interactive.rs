//! CLI entry point for trace2e_proc
//!
//! Supports two execution modes:
//! - Interactive: Read commands from stdin line-by-line
//! - Batch: Read commands from a file
//!
//! # Examples
//!
//! Interactive mode:
//! ```bash
//! ./e2e-proc
//! > OPEN file:///tmp/test.txt
//! > READ file:///tmp/test.txt
//! > ^D
//! ```
//!
//! Batch mode:
//! ```bash
//! ./e2e-proc --playbook scenario.trace2e
//! ```

use clap::Parser;
use std::convert::TryFrom;
use std::io::{self, BufRead, Write};
use trace2e_interactive::{Instruction, IoHandler};
use tracing::{debug, info};
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser, Debug)]
#[command(name = "trace2e-proc")]
#[command(about = "Execute trace2e instruction scenarios", long_about = None)]
struct Args {
    /// Path to a playbook file containing instructions to execute (batch mode)
    #[arg(short, long)]
    playbook: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new("info")).unwrap();
    fmt().with_writer(std::io::stdout).with_target(false).with_env_filter(filter).init();

    let args = Args::parse();

    let mut handler = IoHandler::default();

    if let Some(playbook_path) = args.playbook {
        // Batch mode: read from file
        run_batch_mode(&mut handler, &playbook_path)?;
    } else {
        // Interactive mode: read from stdin
        run_interactive_mode(&mut handler)?;
    }

    Ok(())
}

/// Run in batch mode, reading instructions from a file
fn run_batch_mode(handler: &mut IoHandler, file_path: &str) -> anyhow::Result<()> {
    info!("Running batch mode from file: {}", file_path);

    let file = std::fs::File::open(file_path)?;
    let reader = io::BufReader::new(file);

    let start_time = std::time::Instant::now();
    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        debug!("[{}] {} ... ", line_num + 1, line);
        io::stdout().flush()?;

        match Instruction::try_from(line) {
            Ok(instruction) => {
                if let Err(e) = handler.execute(&instruction) {
                    info!("✗ Error: {}", e);
                    return Err(e);
                }
            }
            Err(e) => {
                info!("✗ Parse error: {}", e);
                return Err(anyhow::anyhow!("{}", e));
            }
        }
    }

    info!(execution_time = ?start_time.elapsed(), "Batch execution completed successfully.");
    Ok(())
}

/// Run in interactive mode, reading instructions from stdin
fn run_interactive_mode(handler: &mut IoHandler) -> anyhow::Result<()> {
    println!("trace2e-proc - Interactive Mode");
    println!("==================================");
    println!("Press Ctrl+D to exit");
    println!();

    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();

    loop {
        print!("> ");
        io::stdout().flush()?;

        line.clear();
        let bytes_read = reader.read_line(&mut line)?;

        // EOF reached
        if bytes_read == 0 {
            println!();
            println!("Goodbye!");
            break;
        }

        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Parse and execute instruction
        match Instruction::try_from(trimmed) {
            Ok(instruction) => {
                if let Err(e) = handler.execute(&instruction) {
                    eprintln!("✗ Error: {}", e);
                    // Continue in interactive mode even after errors
                }
            }
            Err(e) => {
                eprintln!("✗ Parse error: {}", e);
            }
        }
    }

    Ok(())
}
