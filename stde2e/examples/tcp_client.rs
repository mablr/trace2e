use std::{io::Write, net::TcpStream};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use clap::Parser;

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
    let fmt_layer = fmt::layer().with_target(false);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("off"))
        .unwrap();
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    let args = Args::parse();

    let mut stream = TcpStream::connect(args.addr.unwrap_or("127.0.0.1:8888".to_string())).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(args.sleep.unwrap_or(0)));
    stream.write_all(b"\n").unwrap();
    Ok(())
}
