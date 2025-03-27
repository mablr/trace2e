use clap::Parser;
use std::{
    fs::File,
    io::{Read, Write},
};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fmt_layer = fmt::layer().with_target(false);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("off"))
        .unwrap();
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    // Parse command line arguments
    let args = Args::parse();

    // Read from input file if provided
    let content = if let Some(path) = args.input {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        buffer
    } else {
        return Err("No input file provided".into());
    };

    // Write to output file if provided
    if let Some(path) = args.output {
        let mut file = File::create(path)?;
        file.write_all(&content)?;
    } else {
        return Err("No output file provided".into());
    }

    Ok(())
}
