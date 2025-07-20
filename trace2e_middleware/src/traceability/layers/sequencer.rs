use std::{
    collections::{HashMap, VecDeque},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use tokio::{
    join,
    sync::{Mutex, oneshot},
};
use tower::Service;

use crate::traceability::{
    api::{TraceabilityRequest, TraceabilityResponse},
    error::TraceabilityError,
    naming::Identifier,
};

#[derive(Clone)]
pub struct SequencerService<T> {
    inner: T,
    flows: Arc<Mutex<HashMap<Identifier, Identifier>>>,
}

impl<T> SequencerService<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            flows: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Make a flow
    /// Returns the availability state of the source and destination before the attempt
    async fn make_flow(&self, source: Identifier, destination: Identifier) -> (bool, bool) {
        let mut flows = self.flows.lock().await;
        // source is not already reserved by a writer
        let source_available = !flows.contains_key(&source);
        // destination is not already reserved by a reader or writer
        let destination_available =
            !flows.contains_key(&destination) && !flows.values().any(|v| v == &destination);

        // if both are available, create a flow
        if source_available && destination_available {
            flows.insert(destination, source);
        }
        (source_available, destination_available)
    }

    /// Drop a flow
    /// Returns true if the source is not reserved as reader anymore
    async fn drop_flow(&self, source: Identifier, destination: Identifier) -> bool {
        let mut flows = self.flows.lock().await;
        flows.remove(&destination);
        !flows.values().any(|v| *v == source)
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
        let inner_clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, inner_clone);
        let this = self.clone();
        Box::pin(async move {
            match req.clone() {
                TraceabilityRequest::Request {
                    source,
                    destination,
                } => match this.make_flow(source.clone(), destination.clone()).await {
                    (true, true) => inner.call(req).await,
                    (true, false) => Err(TraceabilityError::UnavailableDestination(destination)),
                    (false, true) => Err(TraceabilityError::UnavailableSource(source)),
                    (false, false) => Err(TraceabilityError::UnavailableSourceAndDestination(
                        source,
                        destination,
                    )),
                },
                TraceabilityRequest::Report {
                    source,
                    destination,
                    ..
                } => match inner.call(req).await {
                    Ok(TraceabilityResponse::Ack) => {
                        if !this.drop_flow(source, destination).await {
                            Ok(TraceabilityResponse::Wait)
                        } else {
                            Ok(TraceabilityResponse::Ack)
                        }
                    }
                    Err(e) => Err(e),
                    Ok(_) => unreachable!(),
                },
                _ => Err(TraceabilityError::InvalidRequest),
            }
        })
    }
}

#[derive(Clone)]
pub struct WaitingQueueService<T> {
    inner: T,
    waiting_queue: Arc<Mutex<HashMap<Identifier, VecDeque<oneshot::Sender<()>>>>>,
    max_retries: u32,
}

impl<T> WaitingQueueService<T> {
    pub fn new(inner: T, max_retries: Option<u32>) -> Self {
        Self {
            inner,
            waiting_queue: Arc::new(Mutex::new(HashMap::new())),
            // If None, so the waiting queue is not used
            max_retries: max_retries.unwrap_or_default(),
        }
    }

    async fn join_waiting_queue(&self, id: Identifier) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        if let Some(queue) = self.waiting_queue.lock().await.get_mut(&id) {
            queue.push_back(tx);
        } else {
            let mut queue = VecDeque::new();
            queue.push_back(tx);
            self.waiting_queue.lock().await.insert(id, queue);
        }
        rx
    }

    async fn notify_waiting_queue(&self, id: Identifier) {
        if let Some(queue) = self.waiting_queue.lock().await.get_mut(&id) {
            if let Some(tx) = queue.pop_front() {
                tx.send(()).unwrap();
            }
        }
    }
}

impl<T> Service<TraceabilityRequest> for WaitingQueueService<T>
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
        let inner_clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, inner_clone);
        let max_tries = self.max_retries + 1;
        let this = self.clone();
        Box::pin(async move {
            for _ in 0..max_tries {
                match inner.call(req.clone()).await {
                    Ok(TraceabilityResponse::Grant) => return Ok(TraceabilityResponse::Grant),
                    Ok(TraceabilityResponse::Ack) => match req.clone() {
                        TraceabilityRequest::Report {
                            source,
                            destination,
                            ..
                        } => {
                            join!(
                                this.notify_waiting_queue(source),
                                this.notify_waiting_queue(destination)
                            );
                            return Ok(TraceabilityResponse::Ack);
                        }
                        _ => unreachable!(),
                    },
                    Ok(TraceabilityResponse::Wait) => match req.clone() {
                        TraceabilityRequest::Report { destination, .. } => {
                            this.notify_waiting_queue(destination).await;
                            return Ok(TraceabilityResponse::Ack);
                        }
                        _ => unreachable!(),
                    },
                    Err(TraceabilityError::UnavailableSource(id)) => {
                        let rx = this.join_waiting_queue(id).await;
                        let _ = rx.await;
                    }
                    Err(TraceabilityError::UnavailableDestination(id)) => {
                        let rx = this.join_waiting_queue(id).await;
                        let _ = rx.await;
                    }
                    Err(TraceabilityError::UnavailableSourceAndDestination(id1, id2)) => {
                        let rx1 = this.join_waiting_queue(id1).await;
                        let rx2 = this.join_waiting_queue(id2).await;
                        let (_, _) = join!(rx1, rx2);
                    }
                    Err(e) => return Err(e),
                }
            }
            Err(TraceabilityError::ReachedMaxRetriesWaitingQueue)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tower::{ServiceBuilder, layer::layer_fn, timeout::TimeoutLayer};

    use crate::traceability::{layers::mock_compliance::TraceabilityMockService, naming::Resource};

    use super::*;
    #[tokio::test]
    async fn unit_sequencer_impl_flow() {
        let sequencer = SequencerService::new(());
        let process = Identifier::new("test".to_string(), Resource::new_process(0));
        let file = Identifier::new(
            "test".to_string(),
            Resource::new_file("/tmp/test".to_string()),
        );
        assert_eq!(
            sequencer.make_flow(process.clone(), file.clone()).await,
            (true, true)
        );
        assert_eq!(
            sequencer.drop_flow(process.clone(), file.clone()).await,
            true
        );
        assert_eq!(
            sequencer.make_flow(process.clone(), file.clone()).await,
            (true, true)
        );
        assert_eq!(
            sequencer.drop_flow(process.clone(), file.clone()).await,
            true
        );
    }

    #[tokio::test]
    async fn unit_sequencer_impl_flow_drop_already_dropped() {
        let sequencer = SequencerService::new(());
        let process = Identifier::new("test".to_string(), Resource::new_process(0));
        let file = Identifier::new(
            "test".to_string(),
            Resource::new_file("/tmp/test".to_string()),
        );
        assert_eq!(
            sequencer.make_flow(process.clone(), file.clone()).await,
            (true, true)
        );
        assert_eq!(
            sequencer.drop_flow(process.clone(), file.clone()).await,
            true
        );
        // Already dropped, source is still available so it return true again
        assert_eq!(
            sequencer.drop_flow(process.clone(), file.clone()).await,
            true
        );
    }

    #[tokio::test]
    async fn unit_sequencer_impl_flow_readers_drop() {
        let sequencer = SequencerService::new(());
        let process = Identifier::new("test".to_string(), Resource::new_process(0));
        let file1 = Identifier::new(
            "test".to_string(),
            Resource::new_file("/tmp/test1".to_string()),
        );
        let file2 = Identifier::new(
            "test".to_string(),
            Resource::new_file("/tmp/test2".to_string()),
        );
        let file3 = Identifier::new(
            "test".to_string(),
            Resource::new_file("/tmp/test3".to_string()),
        );
        let file4 = Identifier::new(
            "test".to_string(),
            Resource::new_file("/tmp/test4".to_string()),
        );
        assert_eq!(
            sequencer.make_flow(process.clone(), file1.clone()).await,
            (true, true)
        );
        assert_eq!(
            sequencer.make_flow(process.clone(), file2.clone()).await,
            (true, true)
        );
        assert_eq!(
            sequencer.make_flow(process.clone(), file3.clone()).await,
            (true, true)
        );

        // Must fail because process is already reserved 3 times as reader
        assert_eq!(
            sequencer.make_flow(file4.clone(), process.clone()).await,
            (true, false)
        );

        // Drop 2 reservations
        assert_eq!(
            sequencer.drop_flow(process.clone(), file2.clone()).await,
            false
        );
        assert_eq!(
            sequencer.drop_flow(process.clone(), file1.clone()).await,
            false
        );

        // Must fail because process is still reserved once as reader
        assert_eq!(
            sequencer.make_flow(file4.clone(), process.clone()).await,
            (true, false)
        );

        // Drop last reservation
        assert_eq!(
            sequencer.drop_flow(process.clone(), file3.clone()).await,
            true
        );

        // Must succeed because process is not reserved as reader
        assert_eq!(
            sequencer.make_flow(file4.clone(), process.clone()).await,
            (true, true)
        );
    }

    #[tokio::test]
    async fn unit_sequencer_impl_flow_interference() {
        let sequencer = SequencerService::new(());
        let process1 = Identifier::new("test".to_string(), Resource::new_process(1));
        let process2 = Identifier::new("test".to_string(), Resource::new_process(2));
        let file1 = Identifier::new(
            "test".to_string(),
            Resource::new_file("/tmp/test1".to_string()),
        );
        let file2 = Identifier::new(
            "test".to_string(),
            Resource::new_file("/tmp/test2".to_string()),
        );

        assert_eq!(
            sequencer.make_flow(file1.clone(), process1.clone()).await,
            (true, true)
        );

        // Fails because try get write of write lock
        // (this case may be released in the future, this flow already exists)
        assert_eq!(
            sequencer.make_flow(file1.clone(), process1.clone()).await,
            (true, false)
        );

        // Fails because try get write on read lock
        assert_eq!(
            sequencer.make_flow(process2.clone(), file1.clone()).await,
            (true, false)
        );

        // Fails because try get read on write lock
        assert_eq!(
            sequencer.make_flow(process1.clone(), file2.clone()).await,
            (false, true)
        );

        // Fails because circular flow (get read on write lock & get write on read lock)
        assert_eq!(
            sequencer.make_flow(process1.clone(), file1.clone()).await,
            (false, false)
        );
    }

    #[tokio::test]
    async fn unit_sequencer_layer_flow() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(layer_fn(|inner| SequencerService::new(inner)))
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

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Report {
                    source: process.clone(),
                    destination: file.clone(),
                    success: false,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );
    }

    #[tokio::test]
    async fn unit_sequencer_layer_flow_interference() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(layer_fn(|inner| SequencerService::new(inner)))
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

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: process.clone(),
                    destination: file.clone(),
                })
                .await
                .unwrap_err(),
            TraceabilityError::UnavailableDestination(file.clone())
        );
    }

    #[tokio::test]
    async fn unit_sequencer_layer_flow_circular() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(layer_fn(|inner| SequencerService::new(inner)))
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

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: file.clone(),
                    destination: process.clone(),
                })
                .await
                .unwrap_err(),
            TraceabilityError::UnavailableSourceAndDestination(file, process)
        );
    }

    #[tokio::test]
    async fn unit_sequencer_layer_flow_sequence() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(layer_fn(|inner| SequencerService::new(inner)))
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
            .layer(layer_fn(|inner| SequencerService::new(inner)))
            .service(TraceabilityMockService::default());

        let process1 = Identifier::new(node_id.clone(), Resource::new_process(1));
        let process2 = Identifier::new(node_id.clone(), Resource::new_process(2));
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
                    destination: process1.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );

        // Fails because try get write of write lock
        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: file1.clone(),
                    destination: process1.clone(),
                })
                .await
                .unwrap_err(),
            TraceabilityError::UnavailableDestination(process1.clone())
        );

        // Fails because try get write on read lock
        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: process2.clone(),
                    destination: file1.clone(),
                })
                .await
                .unwrap_err(),
            TraceabilityError::UnavailableDestination(file1.clone())
        );

        // Fails because try get read on write lock
        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: process1.clone(),
                    destination: file2.clone(),
                })
                .await
                .unwrap_err(),
            TraceabilityError::UnavailableSource(process1.clone())
        );

        // Fails because circular flow (get read on write lock & get write on read lock)
        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: process1.clone(),
                    destination: file1.clone(),
                })
                .await
                .unwrap_err(),
            TraceabilityError::UnavailableSourceAndDestination(process1.clone(), file1.clone())
        );
    }
    #[tokio::test]
    async fn unit_sequencer_layer_flow_sequence_interference_multiple_share_releases() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(layer_fn(|inner| SequencerService::new(inner)))
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
        let file3 = Identifier::new(
            node_id.clone(),
            Resource::new_file("/tmp/test3".to_string()),
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

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Request {
                    source: process.clone(),
                    destination: file3.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Report {
                    source: process.clone(),
                    destination: file2.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Wait
        );
        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Report {
                    source: process.clone(),
                    destination: file3.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Wait
        );

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Report {
                    source: process.clone(),
                    destination: file1.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );
    }

    #[tokio::test]
    async fn unit_waiting_queue_layer_flow_interference() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(1)))
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, None)))
            .layer(layer_fn(|inner| SequencerService::new(inner)))
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
    async fn unit_waiting_queue_layer_flow_circular() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(1)))
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, None)))
            .layer(layer_fn(|inner| SequencerService::new(inner)))
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
    async fn unit_waiting_queue_layer_writers_interference_resolution() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(10)))
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, Some(1))))
            .layer(layer_fn(|inner| SequencerService::new(inner)))
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
        });

        tokio::time::sleep(Duration::from_millis(5)).await;
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

        assert_eq!(res.await.unwrap().unwrap(), TraceabilityResponse::Grant);

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
    async fn unit_waiting_queue_layer_writer_readers_interference_resolution() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(2)))
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, Some(1))))
            .layer(layer_fn(|inner| SequencerService::new(inner)))
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

        tokio::time::sleep(Duration::from_millis(1)).await;
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

    #[tokio::test]
    async fn unit_waiting_queue_layer_reader_writer_interference_resolution() {
        let node_id = "test".to_string();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(2)))
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, Some(1))))
            .layer(layer_fn(|inner| SequencerService::new(inner)))
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
                    source: process_clone,
                    destination: file2_clone,
                })
                .await
        });

        tokio::time::sleep(Duration::from_millis(1)).await;
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

        assert_eq!(res.await.unwrap().unwrap(), TraceabilityResponse::Grant);

        assert_eq!(
            sequencer
                .call(TraceabilityRequest::Report {
                    source: process.clone(),
                    destination: file2.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );
    }
}
