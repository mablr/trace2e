use stde2e::{io::Write, net::TcpListener};
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

    let listener = TcpListener::bind("0.0.0.0:8888").unwrap();
    match listener.accept() {
        Ok((mut s, _)) => {
            s.write(b"Hello, world!\n").unwrap();
            Ok(())
        }
        Err(e) => Err(e),
    }
}
