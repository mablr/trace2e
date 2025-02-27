use clap::Parser;
use std::time::Instant;
use stde2e::{fs::File, io::{Read, Write}};

/// File manipulation program that reads from input and writes to output
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input file path
    #[arg(short, long)]
    input: Option<String>,

    /// Output file path
    #[arg(short, long)]
    output: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();
    // Parse command line arguments
    let args = Args::parse();

    // Read from input file if provided
    let content = if let Some(path) = args.input {
        let mut file = File::open(path)?;
        let mut buffer = String::new();
        file.read_to_string(&mut buffer)?;
        buffer
    } else {
        return Err("No input file provided".into());
    };

    // Write to output file if provided
    if let Some(path) = args.output {
        let mut file = File::create(path)?;
        file.write_all(content.as_bytes())?;
    } else {
        return Err("No output file provided".into());
    }

    let end_time = start_time.elapsed();
    println!("\"stde2e\": \"{:?}\",", end_time);
    Ok(())
}
