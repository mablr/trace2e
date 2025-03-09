use std::time::Instant;
use stde2e::{io::Read, net::TcpStream};
use tracing::info;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let fmt_layer = fmt::layer().with_target(false);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("off"))
        .unwrap();
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    let start_connect_time = Instant::now();
    let mut stream = TcpStream::connect("127.0.0.1:8888").unwrap();
    let connect_time = start_connect_time.elapsed();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let mut buf = [0; 16];
    let start_read_time = Instant::now();
    stream.read(&mut buf).unwrap();
    let read_time = start_read_time.elapsed();
    info!(
        "[DEMO] tcp_client_e2e:\t{}",
        connect_time.as_micros() + read_time.as_micros()
    );
    Ok(())
}
