use procfs::process::Process as ProcfsProcess;
use std::{
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::sync::RwLock;
use tower::{BoxError, Service, filter::Predicate};

mod error;
use error::*;

mod message;
use message::*;

mod reservation;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
struct ResourceService {
    prov: Arc<RwLock<Vec<Identifier>>>,
}

impl ResourceService {
    fn new(identifier: Identifier) -> Self {
        Self {
            prov: Arc::new(RwLock::new(vec![identifier])),
        }
    }

    async fn get_prov(&self) -> Vec<Identifier> {
        self.prov.read().await.clone()
    }

    async fn update_prov(&self, prov: Vec<Identifier>) {
        self.prov.write().await.extend(prov);
    }
}

impl Service<ResourceRequest> for ResourceService {
    type Response = ResourceResponse;
    type Error = ResourceError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _request: ResourceRequest) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            match _request {
                ResourceRequest::GetProv => Ok(ResourceResponse::Prov(this.get_prov().await)),
                ResourceRequest::UpdateProv(prov) => {
                    this.update_prov(prov).await;
                    Ok(ResourceResponse::ProvUpdated)
                }
            }
        })
    }
}

#[derive(Default, Debug, Clone)]
struct ProvenanceService;

impl Service<P2mRequest> for ProvenanceService {
    type Response = P2mResponse;
    type Error = P2mError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _request: P2mRequest) -> Self::Future {
        Box::pin(async move { Ok(P2mResponse::Enrolled) })
    }
}

#[derive(Default, Debug, Clone)]
struct P2mValidator;

impl P2mValidator {
    fn is_valid_process(&self, pid: u32) -> bool {
        if let Ok(process) = ProcfsProcess::new(pid.try_into().unwrap()) {
            if let Ok(_) = process.stat() {
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    fn is_valid_stream(&self, local_socket: String, peer_socket: String) -> bool {
        match (
            local_socket.parse::<SocketAddr>(),
            peer_socket.parse::<SocketAddr>(),
        ) {
            (Ok(local_socket), Ok(peer_socket)) => {
                (local_socket.is_ipv4() && peer_socket.is_ipv4())
                    || (local_socket.is_ipv6() && peer_socket.is_ipv6())
            }
            _ => false,
        }
    }
}

impl Predicate<P2mRequest> for P2mValidator {
    type Request = P2mRequest;

    fn check(&mut self, request: Self::Request) -> Result<P2mRequest, BoxError> {
        match request.clone() {
            P2mRequest::RemoteEnroll {
                pid,
                local_socket,
                peer_socket,
                ..
            } => {
                if self.is_valid_process(pid) && self.is_valid_stream(local_socket, peer_socket) {
                    Ok(request)
                } else {
                    Err(Box::new(P2mError::InvalidRequest))
                }
            }
            P2mRequest::LocalEnroll { pid, .. }
            | P2mRequest::IoRequest { pid, .. }
            | P2mRequest::IoReport { pid, .. } => {
                if self.is_valid_process(pid) {
                    Ok(request)
                } else {
                    Err(Box::new(P2mError::InvalidRequest))
                }
            }
        }
    }
}
