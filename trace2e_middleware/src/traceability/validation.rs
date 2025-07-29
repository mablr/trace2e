use std::net::SocketAddr;
use sysinfo::{Pid, System};
use tower::{BoxError, filter::Predicate};

use crate::traceability::{api::P2mRequest, error::TraceabilityError};

#[derive(Default, Debug, Clone)]
pub struct ResourceValidator;

impl ResourceValidator {
    fn is_valid_process(&self, pid: i32) -> bool {
        let mut system = System::new();
        system.refresh_all();
        system.process(Pid::from(pid as usize)).is_some()
    }

    fn is_valid_stream(&self, local_socket: &str, peer_socket: &str) -> bool {
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

    use crate::{
        traceability::{
            api::P2mResponse,
            layers::{
                compliance::ComplianceService, provenance::ProvenanceService,
                sequencer::SequencerService,
            },
            p2m::P2mApiService,
        },
        transport::nop::M2mNop,
    };

    use super::*;

    #[tokio::test]
    async fn unit_traceability_provenance_service_p2m_validator() {
        let mut p2m_service = ServiceBuilder::new()
            .layer(FilterLayer::new(ResourceValidator::default()))
            .service(P2mApiService::new(
                SequencerService::default(),
                ProvenanceService::default(),
                ComplianceService::default(),
                M2mNop::default(),
            ));

        assert_eq!(
            p2m_service
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
            p2m_service
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

        let P2mResponse::Grant(flow_id) = p2m_service
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
            p2m_service
                .call(P2mRequest::IoReport {
                    pid: 1,
                    fd: 1,
                    grant_id: flow_id,
                    result: true
                })
                .await
                .unwrap(),
            P2mResponse::Ack
        );

        assert_eq!(
            p2m_service
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
            p2m_service
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
            p2m_service
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
