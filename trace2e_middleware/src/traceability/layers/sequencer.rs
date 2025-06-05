use std::{
    collections::{HashMap, VecDeque},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use tokio::sync::{Mutex, oneshot};
use tower::Service;

use crate::traceability::{
    error::TraceabilityError,
    message::{TraceabilityRequest, TraceabilityResponse},
    naming::Identifier,
};

#[derive(Clone)]
pub struct SequencerService<T> {
    inner: T,
    flows: Arc<Mutex<HashMap<Identifier, Identifier>>>,
    waiting_queue: Arc<Mutex<HashMap<Identifier, VecDeque<oneshot::Sender<()>>>>>,
    max_retries: u32,
}

impl<T> SequencerService<T> {
    pub fn new(inner: T, max_retries: Option<u32>) -> Self {
        Self {
            inner,
            flows: Arc::new(Mutex::new(HashMap::new())),
            waiting_queue: Arc::new(Mutex::new(HashMap::new())),
            max_retries: max_retries.unwrap_or_default(),
        }
    }

    async fn try_flow(
        &self,
        source: Identifier,
        destination: Identifier,
    ) -> Option<oneshot::Receiver<()>> {
        let mut flows = self.flows.lock().await;
        if flows.contains_key(&source)
            || flows.contains_key(&destination)
            || flows.values().any(|v| v == &destination)
        {
            let (tx, rx) = oneshot::channel();
            if let Some(queue) = self.waiting_queue.lock().await.get_mut(&destination) {
                queue.push_back(tx);
            } else {
                let mut queue = VecDeque::new();
                queue.push_back(tx);
                self.waiting_queue.lock().await.insert(destination, queue);
            }
            Some(rx)
        } else {
            // This will always return None since we've already checked that the key doesn't exist
            flows.insert(destination, source);
            None
        }
    }

    async fn try_lock(&self, destination: Identifier) -> Option<oneshot::Receiver<()>> {
        let mut flows = self.flows.lock().await;
        if flows.contains_key(&destination) || flows.values().any(|v| v == &destination) {
            let (tx, rx) = oneshot::channel();
            if let Some(queue) = self.waiting_queue.lock().await.get_mut(&destination) {
                queue.push_back(tx);
            } else {
                let mut queue = VecDeque::new();
                queue.push_back(tx);
                self.waiting_queue.lock().await.insert(destination, queue);
            }
            Some(rx)
        } else {
            // Insert a None source to take to lock on the destination
            flows.insert(destination, Identifier::new_none());
            None
        }
    }

    async fn drop_flow(&self, destination: Identifier) {
        self.flows.lock().await.remove(&destination);
        if let Some(queue) = self.waiting_queue.lock().await.get_mut(&destination) {
            if let Some(tx) = queue.pop_front() {
                tx.send(()).unwrap();
            }
        }
    }
}

impl<T> Service<TraceabilityRequest> for SequencerService<T>
where
    T: Service<TraceabilityRequest, Response = TraceabilityResponse, Error = TraceabilityError>
        + Clone
        + Sync
        + Send
        + 'static,
    T::Future: Send,
{
    type Response = T::Response;
    type Error = T::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: TraceabilityRequest) -> Self::Future {
        let this = self.clone();
        let inner_clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, inner_clone);
        Box::pin(async move {
            match req.clone() {
                TraceabilityRequest::Request {
                    source,
                    destination,
                } => {
                    for _ in 0..this.max_retries + 1 {
                        if let Some(rx) = this.try_flow(source.clone(), destination.clone()).await {
                            println!("Need to wait for flow {:?} -> {:?}", source, destination);
                            let _ = rx.await; // Wait until the next retry window
                        } else {
                            // Flow is established, call the inner service
                            return inner.call(req).await;
                        }
                    }
                    return Err(TraceabilityError::InconsistentFlow);
                }
                TraceabilityRequest::Report { destination, .. } => {
                    let res = inner.call(req).await;
                    this.drop_flow(destination.clone()).await;
                    res
                }
                _ => inner.call(req).await,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tower::{ServiceBuilder, layer::layer_fn, timeout::TimeoutLayer};

    use crate::traceability::{layers::mock::TraceabilityMockService, naming::Resource};

    use super::*;

    #[tokio::test]
    async fn unit_sequencer_layer_flow_simple() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(1)))
            .layer(layer_fn(|inner| SequencerService::new(inner, None)))
            .service(TraceabilityMockService::default());

        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test".to_string()));

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: process.clone(),
                    destination: file.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );
    }

    #[tokio::test]
    async fn unit_sequencer_layer_flow_interference() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(1)))
            .layer(layer_fn(|inner| SequencerService::new(inner, None)))
            .service(TraceabilityMockService::default());

        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test".to_string()));

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: process.clone(),
                    destination: file.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );

        assert!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: process.clone(),
                    destination: file.clone(),
                })
                .await
                .is_err(),
        );
    }

    #[tokio::test]
    async fn unit_sequencer_layer_flow_circular() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(1)))
            .layer(layer_fn(|inner| SequencerService::new(inner, None)))
            .service(TraceabilityMockService::default());

        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test".to_string()));

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: process.clone(),
                    destination: file.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );

        assert!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: file.clone(),
                    destination: process.clone(),
                })
                .await
                .is_err(),
        );
    }
    #[tokio::test]
    async fn unit_sequencer_layer_flow_shared() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(1)))
            .layer(layer_fn(|inner| SequencerService::new(inner, None)))
            .service(TraceabilityMockService::default());

        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file1 = Identifier::new(
            node_id.clone(),
            Resource::new_file("/tmp/test1".to_string()),
        );
        let file2 = Identifier::new(
            node_id.clone(),
            Resource::new_file("/tmp/test2".to_string()),
        );

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: process.clone(),
                    destination: file1.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: process.clone(),
                    destination: file2.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );
    }

    #[tokio::test]
    async fn unit_sequencer_layer_flow_sequence() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(1)))
            .layer(layer_fn(|inner| SequencerService::new(inner, None)))
            .service(TraceabilityMockService::default());

        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file1 = Identifier::new(
            node_id.clone(),
            Resource::new_file("/tmp/test1".to_string()),
        );
        let file2 = Identifier::new(
            node_id.clone(),
            Resource::new_file("/tmp/test2".to_string()),
        );

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: file1.clone(),
                    destination: process.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Report {
                    source: file1.clone(),
                    destination: process.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: file2.clone(),
                    destination: process.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Report {
                    source: file2.clone(),
                    destination: process.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );
    }

    #[tokio::test]
    async fn unit_sequencer_layer_flow_sequence_interference() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(11)))
            .layer(layer_fn(|inner| SequencerService::new(inner, Some(1))))
            .service(TraceabilityMockService::default());

        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file1 = Identifier::new(
            node_id.clone(),
            Resource::new_file("/tmp/test1".to_string()),
        );
        let file2 = Identifier::new(
            node_id.clone(),
            Resource::new_file("/tmp/test2".to_string()),
        );

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: file1.clone(),
                    destination: process.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );

        let mut sequencer_clone = sequencer.clone();
        let process_clone = process.clone();
        let file2_clone = file2.clone();
        let res = tokio::spawn(async move {
            sequencer_clone
                .call(TraceabilityRequest::Request {
                    source: file2_clone,
                    destination: process_clone,
                })
                .await
                .unwrap()
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Report {
                    source: file1.clone(),
                    destination: process.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );

        assert_eq!(res.await.unwrap(), TraceabilityResponse::Grant);

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Report {
                    source: file2.clone(),
                    destination: process.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );
    }
    #[tokio::test]
    #[ignore] // TODO: Fix this test (notify the waiting queue when all readers of a resource have released it)
    async fn unit_sequencer_layer_flow_sequence_interference_multiple_share_releases() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(100)))
            .layer(layer_fn(|inner| SequencerService::new(inner, Some(1))))
            .service(TraceabilityMockService::default());

        let process1 = Identifier::new(node_id.clone(), Resource::new_process(0));
        let process2 = Identifier::new(node_id.clone(), Resource::new_process(1));
        let process3 = Identifier::new(node_id.clone(), Resource::new_process(2));
        let process4 = Identifier::new(node_id.clone(), Resource::new_process(3));
        let file = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test".to_string()));

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: file.clone(),
                    destination: process1.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: file.clone(),
                    destination: process2.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: file.clone(),
                    destination: process3.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );

        let mut sequencer_clone = sequencer.clone();
        let file_clone = file.clone();
        let process4_clone = process4.clone();
        let res = tokio::spawn(async move {
            sequencer_clone
                .call(TraceabilityRequest::Request {
                    source: process4_clone,
                    destination: file_clone,
                })
                .await
        });

        tokio::time::sleep(Duration::from_millis(2)).await;
        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Report {
                    source: file.clone(),
                    destination: process1.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );
        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Report {
                    source: file.clone(),
                    destination: process2.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );
        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Report {
                    source: file.clone(),
                    destination: process3.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );

        assert_eq!(res.await.unwrap().unwrap(), TraceabilityResponse::Grant);
        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Report {
                    source: process4.clone(),
                    destination: file.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );
    }
}
