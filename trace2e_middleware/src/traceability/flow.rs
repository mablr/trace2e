use std::{collections::HashMap, pin::Pin, sync::Arc, task::{Context, Poll}};

use tokio::sync::Mutex;
use tower::Service;

use super::{error::TraceabilityError, message::{FlowRequest, TraceabilityResponse}, naming::Identifier};

#[derive(Debug, Clone)]
pub struct Flow {
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

#[derive(Debug, Clone)]
pub struct FlowHandlerService {
    flow_id_counter: Arc<Mutex<usize>>,
    flow_map: Arc<Mutex<HashMap<usize, Flow>>>,
}

impl FlowHandlerService {
    pub fn new() -> Self {
        Self {
            flow_id_counter: Arc::new(Mutex::new(0)),
            flow_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Service<FlowRequest> for FlowHandlerService {
    type Response = TraceabilityResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: FlowRequest) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            match request {
                FlowRequest::Request { process, fd, output } => {
                    let flow_id = this.flow_id_counter.lock().await.clone();
                    this.flow_map.lock().await.insert(flow_id, Flow::new(process, fd, output));
                    Ok(TraceabilityResponse::Grant(flow_id))
                }
                FlowRequest::Report { id, success: _ } => {
                    if this.flow_map.lock().await.remove(&id).is_some() {
                        Ok(TraceabilityResponse::Ack)
                    } else {
                        Err(TraceabilityError::NotFoundFlow(id))
                    }
                }
            }
        })
    }
}