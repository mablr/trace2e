use stde2e::{io::Read, net::TcpListener};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use clap::Parser;
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Sleep time in milliseconds
    #[arg(short, long)]
    sleep: Option<u64>,

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
