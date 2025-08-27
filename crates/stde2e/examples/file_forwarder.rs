use std::{
    fs::File,
    io::{Read, Write},
};

use clap::Parser;

/// File manipulation program that reads from input and writes to output
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Sleep time in milliseconds
    #[arg(short, long)]
    read_sleep: Option<u64>,

    /// Sleep time in milliseconds
    #[arg(short, long)]
    write_sleep: Option<u64>,

    /// Input file path
    #[arg(short, long)]
    input: Option<String>,

    /// Output file path
    #[arg(short, long)]
    output: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args = Args::parse();

    // Read from input file if provided
    let content = if let Some(path) = args.input {
        let mut buffer = Vec::new();
        let mut file = File::open(path)?;
        std::thread::sleep(std::time::Duration::from_millis(args.read_sleep.unwrap_or(0)));
        file.read_to_end(&mut buffer)?;
        buffer
    } else {
        return Err("No input file provided".into());
    };

    // Write to output file if provided
    if let Some(path) = args.output {
        let mut file = File::create(path)?;
        std::thread::sleep(std::time::Duration::from_millis(args.write_sleep.unwrap_or(0)));
        file.write_all(&content)?;
    } else {
        return Err("No output file provided".into());
    }

    Ok(())
}
