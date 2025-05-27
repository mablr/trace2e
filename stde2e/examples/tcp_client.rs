use clap::Parser;
use std::{io::Write, net::TcpStream};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Sleep time in milliseconds
    #[arg(short, long)]
    sleep: Option<u64>,

    /// Address to connect to
    #[arg(short, long)]
    addr: Option<String>,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let mut stream = TcpStream::connect(args.addr.unwrap_or("127.0.0.1:8888".to_string())).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(args.sleep.unwrap_or(0)));
    stream.write_all(b"\n").unwrap();
    Ok(())
}
