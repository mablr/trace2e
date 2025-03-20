use std::{io::Read, net::TcpStream};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn main() -> std::io::Result<()> {
    let fmt_layer = fmt::layer().with_target(false);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("off"))
        .unwrap();
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
    let mut stream = TcpStream::connect("127.0.0.1:8888").unwrap();
    let mut buf = [0; 16];
    stream.read(&mut buf).unwrap();
    Ok(())
}
