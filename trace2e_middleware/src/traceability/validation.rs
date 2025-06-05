use std::net::SocketAddr;

use procfs::process::Process as ProcfsProcess;
use tower::{BoxError, filter::Predicate};

use crate::traceability::{error::TraceabilityError, message::P2mRequest};

#[derive(Default, Debug, Clone)]
pub struct ResourceValidator;

impl ResourceValidator {
    fn is_valid_process(&self, pid: i32) -> bool {
        if let Ok(process) = ProcfsProcess::new(pid) {
            process.stat().is_ok()
        } else {
            false
        }
    }

    fn is_valid_stream(&self, local_socket: &String, peer_socket: &String) -> bool {
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

impl Predicate<P2mRequest> for ResourceValidator {
    type Request = P2mRequest;

    fn check(&mut self, request: Self::Request) -> Result<Self::Request, BoxError> {
        match request.clone() {
            P2mRequest::RemoteEnroll {
                pid,
                local_socket,
                peer_socket,
                ..
            } => {
                if self.is_valid_process(pid) {
                    if self.is_valid_stream(&local_socket, &peer_socket) {
                        Ok(request)
                    } else {
                        Err(Box::new(TraceabilityError::InvalidStream(
                            local_socket,
                            peer_socket,
                        )))
                    }
                } else {
                    Err(Box::new(TraceabilityError::InvalidProcess(pid)))
                }
            }
            P2mRequest::LocalEnroll { pid, .. } | P2mRequest::IoRequest { pid, .. } => {
                if self.is_valid_process(pid) {
                    Ok(request)
                } else {
                    Err(Box::new(TraceabilityError::InvalidProcess(pid)))
                }
            }
            P2mRequest::IoReport { .. } => Ok(request),
        }
    }
}

#[cfg(test)]
mod tests {
    use tower::{Service, ServiceBuilder, filter::FilterLayer};

    use crate::traceability::{api::Trace2eService, message::P2mResponse};

    use super::*;

    #[tokio::test]
    async fn unit_traceability_provenance_service_p2m_validator() {
        let validator = ResourceValidator::default();
        let mut provenance_service = ServiceBuilder::new()
            .layer(FilterLayer::new(validator))
            .service(Trace2eService::default());

        assert_eq!(
            provenance_service
                .call(P2mRequest::LocalEnroll {
                    pid: 1,
                    fd: 1,
                    path: "test".to_string()
                })
                .await
                .unwrap(),
            P2mResponse::Ack
        );

        assert_eq!(
            provenance_service
                .call(P2mRequest::RemoteEnroll {
                    pid: 1,
                    local_socket: "127.0.0.1:8080".to_string(),
                    peer_socket: "127.0.0.1:8081".to_string(),
                    fd: 1
                })
                .await
                .unwrap(),
            P2mResponse::Ack
        );

        let P2mResponse::Grant(flow_id) = provenance_service
            .call(P2mRequest::IoRequest {
                pid: 1,
                fd: 1,
                output: true,
            })
            .await
            .unwrap()
        else {
            panic!("Expected P2mResponse::Grant");
        };
        assert_eq!(
            provenance_service
                .call(P2mRequest::IoReport {
                    id: flow_id,
                    success: true
                })
                .await
                .unwrap(),
            P2mResponse::Ack
        );

        assert_eq!(
            provenance_service
                .check(P2mRequest::LocalEnroll {
                    pid: 0,
                    fd: 1,
                    path: "test".to_string()
                })
                .unwrap_err()
                .to_string(),
            "Traceability error, process not found (pid: 0)"
        );

        assert_eq!(
            provenance_service
                .call(P2mRequest::LocalEnroll {
                    pid: 0,
                    fd: 1,
                    path: "test".to_string()
                }) // pid 0 is invalid
                .await
                .unwrap_err()
                .to_string(),
            "Traceability error, process not found (pid: 0)"
        );

        assert_eq!(
            provenance_service
                .call(P2mRequest::RemoteEnroll {
                    pid: 1,
                    local_socket: "bad_socket".to_string(),
                    peer_socket: "bad_socket".to_string(),
                    fd: 1
                })
                .await
                .unwrap_err()
                .to_string(),
            "Traceability error, invalid stream (local_socket: bad_socket, peer_socket: bad_socket)"
        );
    }
}
