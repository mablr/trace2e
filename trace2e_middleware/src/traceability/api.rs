use crate::traceability::{
    error::TraceabilityError,
    message::{TraceabilityRequest, TraceabilityResponse},
};
use procfs::process::Process as ProcfsProcess;
use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc, task::Poll};
use tokio::sync::Mutex;
use tower::Service;

use super::types::{Identifier, Resource};

#[derive(Debug, Clone)]
struct Flow {
    process: Identifier,
    fd: Identifier,
    output: bool,
}

impl Flow {
    pub fn new(process: Identifier, fd: Identifier, output: bool) -> Self {
        Self {
            process,
            fd,
            output,
        }
    }
}

fn instantiate_process_resource(
    pid: i32,
    strict_mode: bool,
) -> Result<Resource, TraceabilityError> {
    if strict_mode {
        match ProcfsProcess::new(pid) {
            Ok(procfs_process) => {
                let starttime = procfs_process
                    .stat()
                    .map_err(|_| TraceabilityError::InconsistentProcess(pid))?
                    .starttime;
                let exe_path = procfs_process
                    .exe()
                    .map_err(|_| TraceabilityError::InconsistentProcess(pid))?
                    .to_str()
                    .unwrap_or_default()
                    .to_string();
                Ok(Resource::new_process(pid, starttime, exe_path))
            }
            Err(_) => Err(TraceabilityError::NotFoundProcess(pid)),
        }
    } else {
        Ok(Resource::new_process(pid, 0, String::default()))
    }
}

#[derive(Debug, Clone)]
pub struct TraceabilityApiService {
    node_id: String,
    fd_map: Arc<Mutex<HashMap<(i32, i32), Resource>>>, // Resource to be replaced by Identifier later
    process_map: Arc<Mutex<HashMap<i32, Resource>>>,
    flow_id_counter: Arc<Mutex<usize>>,
    flow_map: Arc<Mutex<HashMap<usize, Flow>>>,
    strict_mode: bool,
}

impl Default for TraceabilityApiService {
    fn default() -> Self {
        Self::new_with_node_id(
            rustix::system::uname()
                .nodename()
                .to_str()
                .unwrap()
                .to_string()
                .to_lowercase(),
        )
    }
}

impl TraceabilityApiService {
    pub fn new_with_node_id(node_id: String) -> Self {
        Self {
            node_id,
            fd_map: Arc::new(Mutex::new(HashMap::new())),
            process_map: Arc::new(Mutex::new(HashMap::new())),
            flow_id_counter: Arc::new(Mutex::new(0)),
            flow_map: Arc::new(Mutex::new(HashMap::new())),
            strict_mode: false,
        }
    }

    pub fn set_strict_mode(mut self) -> Self {
        self.strict_mode = true;
        self
    }
}

impl Service<TraceabilityRequest> for TraceabilityApiService {
    type Response = TraceabilityResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<TraceabilityResponse, TraceabilityError>>>>;

    fn poll_ready(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(())) // TODO: Implement readiness check
    }

    fn call(&mut self, request: TraceabilityRequest) -> Self::Future {
        let self_clone = self.clone();
        Box::pin(async move {
            match request {
                TraceabilityRequest::LocalEnroll { pid, fd, path } => {
                    self_clone.process_map.lock().await.insert(
                        pid,
                        instantiate_process_resource(pid, self_clone.strict_mode)?,
                    );
                    self_clone
                        .fd_map
                        .lock()
                        .await
                        .insert((pid, fd), Resource::new_file(path));
                    Ok(TraceabilityResponse::Ack)
                }
                TraceabilityRequest::RemoteEnroll {
                    pid,
                    fd,
                    local_socket,
                    peer_socket,
                } => {
                    self_clone.process_map.lock().await.insert(
                        pid,
                        instantiate_process_resource(pid, self_clone.strict_mode)?,
                    );
                    self_clone
                        .fd_map
                        .lock()
                        .await
                        .insert((pid, fd), Resource::new_stream(local_socket, peer_socket));
                    Ok(TraceabilityResponse::Ack)
                }
                TraceabilityRequest::IoRequest { pid, fd, output } => {
                    // Dummy implementation for now
                    if let (Some(process), Some(fd)) = (
                        self_clone.process_map.lock().await.get(&pid),
                        self_clone.fd_map.lock().await.get(&(pid, fd)),
                    ) {
                        let flow = Flow::new(
                            Identifier {
                                node: self_clone.node_id.clone(),
                                resource: process.clone(),
                            },
                            Identifier {
                                node: self_clone.node_id.clone(),
                                resource: fd.clone(),
                            },
                            output,
                        );
                        let mut flow_map = self_clone.flow_map.lock().await;
                        let mut flow_id_counter = self_clone.flow_id_counter.lock().await;
                        let flow_id = *flow_id_counter;
                        *flow_id_counter += 1;
                        flow_map.insert(flow_id, flow);
                        Ok(TraceabilityResponse::Grant(flow_id))
                    } else {
                        Err(TraceabilityError::UndeclaredResource(pid, fd))
                    }
                }
                TraceabilityRequest::IoReport { id, success: _ } => {
                    // Dummy implementation for now
                    let mut flow_map = self_clone.flow_map.lock().await;
                    if flow_map.remove(&id).is_some() {
                        Ok(TraceabilityResponse::Ack)
                    } else {
                        Err(TraceabilityError::NotFoundFlow(id))
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use tower::ServiceBuilder;

    use super::*;

    #[tokio::test]
    async fn unit_traceability_api_request_response() {
        let traceability_api = TraceabilityApiService::new_with_node_id("test".to_string());
        let mut traceability_api_service = ServiceBuilder::new().service(traceability_api);

        assert_eq!(
            traceability_api_service
                .call(TraceabilityRequest::LocalEnroll {
                    pid: 1,
                    fd: 3,
                    path: "/tmp/test.txt".to_string()
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );
        assert_eq!(
            traceability_api_service
                .call(TraceabilityRequest::RemoteEnroll {
                    pid: 1,
                    fd: 3,
                    local_socket: "127.0.0.1:8080".to_string(),
                    peer_socket: "127.0.0.1:8081".to_string()
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );

        assert_eq!(
            traceability_api_service
                .call(TraceabilityRequest::IoRequest {
                    pid: 1,
                    fd: 3,
                    output: true
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant(0)
        );
        assert_eq!(
            traceability_api_service
                .call(TraceabilityRequest::IoReport {
                    id: 0,
                    success: true
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );

        assert_eq!(
            traceability_api_service
                .call(TraceabilityRequest::IoRequest {
                    pid: 1,
                    fd: 3,
                    output: false
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant(1)
        );
        assert_eq!(
            traceability_api_service
                .call(TraceabilityRequest::IoReport {
                    id: 1,
                    success: true
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );
    }

    #[tokio::test]
    async fn unit_traceability_api_strict_mode() {
        let traceability_api =
            TraceabilityApiService::new_with_node_id("test".to_string()).set_strict_mode();
        let mut traceability_api_service = ServiceBuilder::new().service(traceability_api);

        // Test not found process
        assert_eq!(
            traceability_api_service
                .call(TraceabilityRequest::LocalEnroll {
                    pid: 0,
                    fd: 3,
                    path: "/tmp/test.txt".to_string()
                })
                .await
                .unwrap_err(),
            TraceabilityError::NotFoundProcess(0)
        );

        // Test inconsistent process
        assert_eq!(
            traceability_api_service
                .call(TraceabilityRequest::LocalEnroll {
                    pid: 1,
                    fd: 3,
                    path: "/tmp/test.txt".to_string()
                })
                .await
                .unwrap_err(),
            TraceabilityError::InconsistentProcess(1)
        );

        // Test successful process instantiation with strict mode
        assert_eq!(
            traceability_api_service
                .call(TraceabilityRequest::LocalEnroll {
                    pid: std::process::id() as i32,
                    fd: 3,
                    path: "/tmp/test.txt".to_string()
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );
    }

    #[tokio::test]
    async fn unit_traceability_api_io_invalid_request() {
        let traceability_api = TraceabilityApiService::new_with_node_id("test".to_string());
        let mut traceability_api_service = ServiceBuilder::new().service(traceability_api);

        // Neither process nor fd are enrolled
        assert_eq!(
            traceability_api_service
                .call(TraceabilityRequest::IoRequest {
                    pid: std::process::id() as i32,
                    fd: 3,
                    output: true,
                })
                .await
                .unwrap_err(),
            TraceabilityError::UndeclaredResource(std::process::id() as i32, 3)
        );

        traceability_api_service
            .call(TraceabilityRequest::LocalEnroll {
                pid: std::process::id() as i32,
                fd: 4,
                path: "/tmp/test.txt".to_string(),
            })
            .await
            .unwrap();

        // Only process is enrolled
        assert_eq!(
            traceability_api_service
                .call(TraceabilityRequest::IoRequest {
                    pid: std::process::id() as i32,
                    fd: 3,
                    output: true,
                })
                .await
                .unwrap_err(),
            TraceabilityError::UndeclaredResource(std::process::id() as i32, 3)
        );
    }

    #[tokio::test]
    async fn unit_traceability_api_io_invalid_report() {
        let traceability_api = TraceabilityApiService::new_with_node_id("test".to_string());
        let mut traceability_api_service = ServiceBuilder::new().service(traceability_api);

        // Invalid grant id
        assert_eq!(
            traceability_api_service
                .call(TraceabilityRequest::IoReport {
                    id: 0,
                    success: true,
                })
                .await
                .unwrap_err(),
            TraceabilityError::NotFoundFlow(0)
        );
    }
}
