//! A distributed traceability middleware.
//!
//! This software provides a mediation layer that provides provenance recording
//! and compliance enforcement on I/O objects such as files or streams. The use
//! of a custom I/O library is required to wrap standard I/O library methods to
//! make input/output conditional on middleware authorization.
//!
//! A unique instance of this software should run on each node where
//! traceability is enforced.
//!
//! Process/middleware and middleware/middleware communication relies on [`tonic`],
//! a Rust implementation of gRPC, a high performance, open source RPC framework.
//!
//! [`tonic`]: https://docs.rs/tonic

#[cfg(test)]
pub mod tests;

pub mod traceability;
pub mod transport;

pub mod trace2e_tracing {
    use std::sync::{Once, OnceLock};

    use opentelemetry_sdk::logs::SdkLoggerProvider;
    use tracing_subscriber::{
        EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt,
    };

    static INIT: Once = Once::new();
    static LOGGER_PROVIDER: OnceLock<SdkLoggerProvider> = OnceLock::new();

    pub fn init() {
        INIT.call_once(|| {
            let filter =
                EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new("off")).unwrap();

            if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
                let exporter = opentelemetry_otlp::LogExporter::builder()
                    .with_tonic()
                    .build()
                    .expect("failed to build OTLP log exporter");

                let provider = SdkLoggerProvider::builder().with_batch_exporter(exporter).build();

                let otel_layer =
                    opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
                        &provider,
                    );
                let otel_filter = EnvFilter::new("info")
                    .add_directive("hyper=off".parse().unwrap())
                    .add_directive("tonic=off".parse().unwrap())
                    .add_directive("h2=off".parse().unwrap())
                    .add_directive("reqwest=off".parse().unwrap());

                let fmt_layer = fmt::layer().with_target(false);

                tracing_subscriber::registry()
                    .with(otel_layer.with_filter(otel_filter))
                    .with(fmt_layer.with_filter(filter))
                    .init();

                let _ = LOGGER_PROVIDER.set(provider);
            } else {
                fmt().with_target(false).with_test_writer().with_env_filter(filter).init();
            }
        });
    }

    pub fn shutdown() {
        if let Some(provider) = LOGGER_PROVIDER.get()
            && let Err(e) = provider.shutdown()
        {
            eprintln!("failed to shut down OTEL logger provider: {e}");
        }
    }
}
