use std::{io::Read, net::TcpListener};
use clap::Parser;
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Sleep time in milliseconds
    #[arg(short, long)]
    sleep: Option<u64>,

}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let listener = TcpListener::bind("0.0.0.0:8888").unwrap();
    match listener.accept() {
        Ok((mut stream, _)) => {
            std::thread::sleep(std::time::Duration::from_millis(args.sleep.unwrap_or(0)));
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).unwrap();
            Ok(())
        }
        Err(e) => Err(e),
    }
}
