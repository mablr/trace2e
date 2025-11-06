//! Macros for common demo operations

/// Enroll a file locally with the P2M service
#[macro_export]
macro_rules! local_enroll {
    ($p2m:expr, $mapping:expr) => {
        match $p2m
            .call(trace2e_core::traceability::api::P2mRequest::LocalEnroll {
                pid: $mapping.pid(),
                fd: $mapping.fd(),
                path: $mapping.file_path(),
            })
            .await
        {
            Ok(trace2e_core::traceability::api::P2mResponse::Ack) => {
                tracing::info!(
                    pid = $mapping.pid(),
                    fd = $mapping.fd(),
                    path = %$mapping.file_path(),
                    "File enrolled locally"
                );
            }
            Err(e) => {
                tracing::error!(
                    pid = $mapping.pid(),
                    fd = $mapping.fd(),
                    path = %$mapping.file_path(),
                    error = ?e,
                    "Failed to enroll file"
                );
                panic!("Failed to enroll file: {:?}", e);
            }
            _ => panic!("Unexpected response from LocalEnroll"),
        }
    };
}

/// Enroll a stream remotely with the P2M service
#[macro_export]
macro_rules! remote_enroll {
    ($p2m:expr, $mapping:expr) => {
        match $p2m
            .call(trace2e_core::traceability::api::P2mRequest::RemoteEnroll {
                pid: $mapping.pid(),
                fd: $mapping.fd(),
                local_socket: $mapping.stream_local_socket(),
                peer_socket: $mapping.stream_peer_socket(),
            })
            .await
        {
            Ok(trace2e_core::traceability::api::P2mResponse::Ack) => {
                tracing::info!(
                    pid = $mapping.pid(),
                    fd = $mapping.fd(),
                    local = %$mapping.stream_local_socket(),
                    peer = %$mapping.stream_peer_socket(),
                    "Stream enrolled remotely"
                );
            }
            Err(e) => {
                tracing::error!(
                    pid = $mapping.pid(),
                    fd = $mapping.fd(),
                    error = ?e,
                    "Failed to enroll stream"
                );
                panic!("Failed to enroll stream: {:?}", e);
            }
            _ => panic!("Unexpected response from RemoteEnroll"),
        }
    };
}

/// Request write permission for a resource
#[macro_export]
macro_rules! write_request {
    ($p2m:expr, $mapping:expr) => {
        match $p2m
            .call(trace2e_core::traceability::api::P2mRequest::IoRequest {
                pid: $mapping.pid(),
                fd: $mapping.fd(),
                output: true,
            })
            .await
        {
            Ok(trace2e_core::traceability::api::P2mResponse::Grant(flow_id)) => {
                tracing::debug!(
                    pid = $mapping.pid(),
                    fd = $mapping.fd(),
                    flow_id = %flow_id,
                    "Write request granted"
                );
                flow_id
            }
            Err(e) => {
                tracing::warn!(
                    pid = $mapping.pid(),
                    fd = $mapping.fd(),
                    error = ?e,
                    "Write request denied by policy"
                );
                u128::MAX
            }
            _ => panic!("Unexpected response from IoRequest"),
        }
    };
}

/// Request read permission for a resource
#[macro_export]
macro_rules! read_request {
    ($p2m:expr, $mapping:expr) => {
        match $p2m
            .call(trace2e_core::traceability::api::P2mRequest::IoRequest {
                pid: $mapping.pid(),
                fd: $mapping.fd(),
                output: false,
            })
            .await
        {
            Ok(trace2e_core::traceability::api::P2mResponse::Grant(flow_id)) => {
                tracing::debug!(
                    pid = $mapping.pid(),
                    fd = $mapping.fd(),
                    flow_id = %flow_id,
                    "Read request granted"
                );
                flow_id
            }
            Err(e) => {
                tracing::warn!(
                    pid = $mapping.pid(),
                    fd = $mapping.fd(),
                    error = ?e,
                    "Read request denied by policy"
                );
                u128::MAX
            }
            _ => panic!("Unexpected response from IoRequest"),
        }
    };
}

/// Report the result of an I/O operation
#[macro_export]
macro_rules! io_report {
    ($p2m:expr, $mapping:expr, $flow_id:expr, $result:expr) => {
        if $flow_id != u128::MAX {
            match $p2m
                .call(trace2e_core::traceability::api::P2mRequest::IoReport {
                    pid: $mapping.pid(),
                    fd: $mapping.fd(),
                    grant_id: $flow_id,
                    result: $result,
                })
                .await
            {
                Ok(trace2e_core::traceability::api::P2mResponse::Ack) => {
                    tracing::debug!(
                        pid = $mapping.pid(),
                        fd = $mapping.fd(),
                        flow_id = %$flow_id,
                        result = %$result,
                        "I/O operation completed"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        pid = $mapping.pid(),
                        fd = $mapping.fd(),
                        error = ?e,
                        "Failed to report I/O operation"
                    );
                }
                _ => panic!("Unexpected response from IoReport"),
            }
        }
    };
}

/// Perform a read operation (request + report)
#[macro_export]
macro_rules! demo_read {
    ($p2m:expr, $mapping:expr) => {
        let flow_id = $crate::read_request!($p2m, $mapping);
        $crate::io_report!($p2m, $mapping, flow_id, true);
    };
}

/// Perform a write operation (request + report)
#[macro_export]
macro_rules! demo_write {
    ($p2m:expr, $mapping:expr) => {
        let flow_id = $crate::write_request!($p2m, $mapping);
        $crate::io_report!($p2m, $mapping, flow_id, true);
    };
}

/// Broadcast deletion of a resource to all nodes
#[macro_export]
macro_rules! broadcast_deletion {
    ($o2m:expr, $resource:expr) => {
        match $o2m
            .call(trace2e_core::traceability::api::O2mRequest::BroadcastDeletion($resource.clone()))
            .await
        {
            Ok(trace2e_core::traceability::api::O2mResponse::Ack) => {
                tracing::info!(resource = %$resource, "Deletion broadcasted successfully");
            }
            Err(e) => {
                tracing::error!(resource = %$resource, error = ?e, "Failed to broadcast deletion");
                panic!("Failed to broadcast deletion: {:?}", e);
            }
            _ => panic!("Unexpected response from BroadcastDeletion"),
        }
    };
}

/// Enable consent enforcement on a resource
#[macro_export]
macro_rules! enforce_consent {
    ($o2m:expr, $resource:expr) => {
        match $o2m
            .call(trace2e_core::traceability::api::O2mRequest::EnforceConsent($resource.clone()))
            .await
        {
            Ok(trace2e_core::traceability::api::O2mResponse::Notifications(rx)) => {
                tracing::info!(resource = %$resource, "Consent enforcement enabled");
                rx
            }
            Err(e) => {
                tracing::error!(resource = %$resource, error = ?e, "Failed to enforce consent");
                panic!("Failed to enforce consent: {:?}", e);
            }
            _ => panic!("Unexpected response from EnforceConsent"),
        }
    };
}

/// Set a consent decision for a resource flow
#[macro_export]
macro_rules! set_consent_decision {
    ($o2m:expr, $resource:expr, $destination:expr, $decision:expr) => {
        match $o2m
            .call(trace2e_core::traceability::api::O2mRequest::SetConsentDecision {
                source: $resource.clone(),
                destination: $destination.clone(),
                decision: $decision,
            })
            .await
        {
            Ok(trace2e_core::traceability::api::O2mResponse::Ack) => {
                tracing::info!(
                    resource = %$resource,
                    destination = ?$destination,
                    decision = $decision,
                    "Consent decision recorded"
                );
            }
            Err(e) => {
                tracing::error!(
                    resource = %$resource,
                    destination = ?$destination,
                    error = ?e,
                    "Failed to set consent decision"
                );
                panic!("Failed to set consent decision: {:?}", e);
            }
            _ => panic!("Unexpected response from SetConsentDecision"),
        }
    };
}
