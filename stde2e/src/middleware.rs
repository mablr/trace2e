use tonic::transport::Channel;
use once_cell::sync::Lazy;

tonic::include_proto!("p2m_api");

pub(crate) static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap()
});

pub(crate) static GRPC_CLIENT: Lazy<p2m_client::P2mClient<Channel>> = Lazy::new(|| {
    let rt = &*TOKIO_RUNTIME;
    rt.block_on(p2m_client::P2mClient::connect("http://[::1]:8080")).unwrap()
});