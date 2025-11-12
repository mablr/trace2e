//! CLI entry point for trace2e_runner
//!
//! Supports two execution modes:
//! - Interactive: Read commands from stdin line-by-line
//! - Batch: Read commands from a file
//!
//! # Examples
//!
//! Interactive mode:
//! ```bash
//! trace2e-runner
//! > OPEN file:///tmp/test.txt
//! > READ file:///tmp/test.txt
//! > ^D
//! ```
//!
//! Batch mode:
//! ```bash
//! ./trace2e-runner --file scenario.txt
//! ```

use clap::Parser;
use std::convert::TryFrom;
use std::io::{self, BufRead, Write};
use trace2e_runner::{Instruction, IoHandler};

#[derive(Parser, Debug)]
#[command(name = "trace2e-runner")]
#[command(about = "Execute trace2e instruction scenarios", long_about = None)]
struct Args {
    /// Path to a playbook file containing instructions to execute (batch mode)
    #[arg(short, long)]
    playbook: Option<String>,
}

fn main() -> anyhow::Result<()> {
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
    println!("Running batch mode from file: {}", file_path);
    println!();

    let file = std::fs::File::open(file_path)?;
    let reader = io::BufReader::new(file);

    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        print!("[{}] {} ... ", line_num + 1, line);
        io::stdout().flush()?;

        match Instruction::try_from(line) {
            Ok(instruction) => {
                if let Err(e) = handler.execute(&instruction) {
                    println!("✗ Error: {}", e);
                    return Err(e);
                }
            }
            Err(e) => {
                println!("✗ Parse error: {}", e);
                return Err(anyhow::anyhow!("{}", e));
            }
        }
    }

    println!();
    println!("Batch execution completed successfully.");
    Ok(())
}

/// Run in interactive mode, reading instructions from stdin
fn run_interactive_mode(handler: &mut IoHandler) -> anyhow::Result<()> {
    println!("trace2e-runner - Interactive Mode");
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
