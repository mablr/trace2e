use std::{collections::VecDeque, pin::Pin, sync::Arc, task::Poll};

use dashmap::DashMap;
use tokio::{join, sync::oneshot};
use tower::Service;
#[cfg(feature = "trace2e_tracing")]
use tracing::{debug, info};

use crate::traceability::{
    api::{SequencerRequest, SequencerResponse},
    error::TraceabilityError,
    naming::Resource,
};

#[derive(Clone, Default)]
pub struct SequencerService {
    flows: Arc<DashMap<Resource, Resource>>,
}

impl SequencerService {
    /// Make a flow
    /// Returns the availability state of the source and destination before the attempt
    async fn make_flow(
        &self,
        source: Resource,
        destination: Resource,
    ) -> Result<SequencerResponse, TraceabilityError> {
        // source is not already reserved by a writer
        let source_available = !self.flows.contains_key(&source);
        // destination is not already reserved by a reader or writer
        let destination_available = !self.flows.contains_key(&destination)
            && !self.flows.iter().any(|entry| entry.value() == &destination);

        // if both are available, create a flow
        if source_available && destination_available {
            self.flows.insert(destination, source);
            Ok(SequencerResponse::FlowReserved)
        } else if source_available {
            Err(TraceabilityError::UnavailableDestination(destination))
        } else if destination_available {
            Err(TraceabilityError::UnavailableSource(source))
        } else {
            Err(TraceabilityError::UnavailableSourceAndDestination(source, destination))
        }
    }

    /// Drop a flow
    /// Returns the SequencerResponse to the caller
    async fn drop_flow(&self, destination: &Resource) -> SequencerResponse {
        if let Some((_, source)) = self.flows.remove(destination) {
            if self.flows.iter().any(|entry| entry.value() == &source) {
                // Partial release of the flow
                // source is still reserved as reader, notify the waiting queue of the destination
                SequencerResponse::FlowReleased {
                    source: None,
                    destination: Some(destination.clone()),
                }
            } else {
                // Complete release of the flow
                // both source and destination are available again, notify both waiting queues
                SequencerResponse::FlowReleased {
                    source: Some(source),
                    destination: Some(destination.clone()),
                }
            }
        } else {
            // Destination is not reserved, nothing to do
            SequencerResponse::FlowReleased { source: None, destination: None }
        }
    }
}

impl Service<SequencerRequest> for SequencerService {
    type Response = SequencerResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: SequencerRequest) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            match request {
                SequencerRequest::ReserveFlow { source, destination } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[sequencer] ReserveFlow: source: {:?}, destination: {:?}",
                        source, destination
                    );
                    this.make_flow(source, destination).await
                }
                SequencerRequest::ReleaseFlow { destination } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[sequencer] ReleaseFlow: destination: {:?}", destination);
                    Ok(this.drop_flow(&destination).await)
                }
            }
        })
    }
}

#[derive(Clone)]
pub struct WaitingQueueService<T> {
    inner: T,
    waiting_queue: Arc<DashMap<Resource, VecDeque<oneshot::Sender<()>>>>,
    max_retries: u32,
}

impl<T> WaitingQueueService<T> {
    pub fn new(inner: T, max_retries: Option<u32>) -> Self {
        Self {
            inner,
            waiting_queue: Arc::new(DashMap::new()),
            // If None, so the waiting queue is not used
            max_retries: max_retries.unwrap_or_default(),
        }
    }

    async fn join_waiting_queue(&self, resource: &Resource) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        if let Some(mut queue) = self.waiting_queue.get_mut(resource) {
            queue.push_back(tx);
        } else {
            let mut queue = VecDeque::new();
            queue.push_back(tx);
            self.waiting_queue.insert(resource.clone(), queue);
        }
        rx
    }

    async fn notify_waiting_queue(&self, resource: &Option<Resource>) {
        if let Some(resource) = resource
            && let Some(mut queue) = self.waiting_queue.get_mut(resource)
            && let Some(tx) = queue.pop_front()
        {
            tx.send(()).unwrap();
        }
    }
}

impl<T> Service<SequencerRequest> for WaitingQueueService<T>
where
    T: Service<SequencerRequest, Response = SequencerResponse, Error = TraceabilityError>
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

    fn call(&mut self, req: SequencerRequest) -> Self::Future {
        let mut inner = self.inner.clone();
        let max_tries = self.max_retries + 1;
        let this = self.clone();
        Box::pin(async move {
            for _ in 0..max_tries {
                match inner.call(req.clone()).await {
                    Ok(SequencerResponse::FlowReserved) => {
                        #[cfg(feature = "trace2e_tracing")]
                        debug!("[sequencer] FlowReserved");
                        return Ok(SequencerResponse::FlowReserved);
                    }
                    Ok(SequencerResponse::FlowReleased { source, destination }) => {
                        join!(
                            this.notify_waiting_queue(&source),
                            this.notify_waiting_queue(&destination)
                        );
                        #[cfg(feature = "trace2e_tracing")]
                        debug!(
                            "[sequencer] FlowReleased: source: {:?}, destination: {:?}",
                            source, destination
                        );
                        return Ok(SequencerResponse::FlowReleased { source, destination });
                    }
                    Err(TraceabilityError::UnavailableSource(source)) => {
                        let rx = this.join_waiting_queue(&source).await;
                        #[cfg(feature = "trace2e_tracing")]
                        debug!("[sequencer] waiting source: {:?}", source);
                        let _ = rx.await;
                    }
                    Err(TraceabilityError::UnavailableDestination(destination)) => {
                        let rx = this.join_waiting_queue(&destination).await;
                        #[cfg(feature = "trace2e_tracing")]
                        debug!("[sequencer] waiting destination: {:?}", destination);
                        let _ = rx.await;
                    }
                    Err(TraceabilityError::UnavailableSourceAndDestination(
                        source,
                        destination,
                    )) => {
                        let rx1 = this.join_waiting_queue(&source).await;
                        let rx2 = this.join_waiting_queue(&destination).await;
                        #[cfg(feature = "trace2e_tracing")]
                        debug!(
                            "[sequencer] waiting source: {:?}, destination: {:?}",
                            source, destination
                        );
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

    use super::*;
    use crate::traceability::naming::Resource;
    #[tokio::test]
    async fn unit_sequencer_impl_flow() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let sequencer = SequencerService::default();
        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());
        assert_eq!(
            sequencer.make_flow(process.clone(), file.clone()).await,
            Ok(SequencerResponse::FlowReserved)
        );
        assert_eq!(
            sequencer.drop_flow(&file).await,
            SequencerResponse::FlowReleased {
                source: Some(process.clone()),
                destination: Some(file.clone())
            }
        );
        assert_eq!(
            sequencer.make_flow(process.clone(), file.clone()).await,
            Ok(SequencerResponse::FlowReserved)
        );
        assert_eq!(
            sequencer.drop_flow(&file).await,
            SequencerResponse::FlowReleased { source: Some(process), destination: Some(file) }
        );
    }

    #[tokio::test]
    async fn unit_sequencer_impl_flow_drop_already_dropped() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let sequencer = SequencerService::default();
        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());
        assert_eq!(
            sequencer.make_flow(process.clone(), file.clone()).await,
            Ok(SequencerResponse::FlowReserved)
        );
        assert_eq!(
            sequencer.drop_flow(&file).await,
            SequencerResponse::FlowReleased {
                source: Some(process),
                destination: Some(file.clone())
            }
        );
        // Already dropped, source is still available so it return true again
        assert_eq!(
            sequencer.drop_flow(&file).await,
            SequencerResponse::FlowReleased { source: None, destination: None }
        );
    }

    #[tokio::test]
    async fn unit_sequencer_impl_flow_readers_drop() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let sequencer = SequencerService::default();
        let process = Resource::new_process_mock(0);
        let file1 = Resource::new_file("/tmp/test1".to_string());
        let file2 = Resource::new_file("/tmp/test2".to_string());
        let file3 = Resource::new_file("/tmp/test3".to_string());
        let file4 = Resource::new_file("/tmp/test4".to_string());
        assert_eq!(
            sequencer.make_flow(process.clone(), file1.clone()).await,
            Ok(SequencerResponse::FlowReserved)
        );
        assert_eq!(
            sequencer.make_flow(process.clone(), file2.clone()).await,
            Ok(SequencerResponse::FlowReserved)
        );
        assert_eq!(
            sequencer.make_flow(process.clone(), file3.clone()).await,
            Ok(SequencerResponse::FlowReserved)
        );

        // Must fail because process is already reserved 3 times as reader
        assert_eq!(
            sequencer.make_flow(file4.clone(), process.clone()).await,
            Err(TraceabilityError::UnavailableDestination(process.clone()))
        );

        // Drop 2 reservations
        assert_eq!(
            sequencer.drop_flow(&file2).await,
            SequencerResponse::FlowReleased { source: None, destination: Some(file2) }
        );
        assert_eq!(
            sequencer.drop_flow(&file1).await,
            SequencerResponse::FlowReleased { source: None, destination: Some(file1) }
        );

        // Must fail because process is still reserved once as reader
        assert_eq!(
            sequencer.make_flow(file4.clone(), process.clone()).await,
            Err(TraceabilityError::UnavailableDestination(process.clone()))
        );

        // Drop last reservation
        assert_eq!(
            sequencer.drop_flow(&file3).await,
            SequencerResponse::FlowReleased {
                source: Some(process.clone()),
                destination: Some(file3)
            }
        );

        // Must succeed because process is not reserved as reader
        assert_eq!(sequencer.make_flow(file4, process).await, Ok(SequencerResponse::FlowReserved));
    }

    #[tokio::test]
    async fn unit_sequencer_impl_flow_interference() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let sequencer = SequencerService::default();
        let process1 = Resource::new_process_mock(1);
        let process2 = Resource::new_process_mock(2);
        let file1 = Resource::new_file("/tmp/test1".to_string());
        let file2 = Resource::new_file("/tmp/test2".to_string());

        assert_eq!(
            sequencer.make_flow(file1.clone(), process1.clone()).await,
            Ok(SequencerResponse::FlowReserved)
        );

        // Fails because try get write of write lock
        // (this case may be released in the future, this flow already exists)
        assert_eq!(
            sequencer.make_flow(file1.clone(), process1.clone()).await,
            Err(TraceabilityError::UnavailableDestination(process1.clone()))
        );

        // Fails because try get write on read lock
        assert_eq!(
            sequencer.make_flow(process2, file1.clone()).await,
            Err(TraceabilityError::UnavailableDestination(file1.clone()))
        );

        // Fails because try get read on write lock
        assert_eq!(
            sequencer.make_flow(process1.clone(), file2).await,
            Err(TraceabilityError::UnavailableSource(process1.clone()))
        );

        // Fails because circular flow (get read on write lock & get write on read lock)
        assert_eq!(
            sequencer.make_flow(process1.clone(), file1.clone()).await,
            Err(TraceabilityError::UnavailableSourceAndDestination(process1, file1))
        );
    }

    #[tokio::test]
    async fn unit_sequencer_layer_flow() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut sequencer = SequencerService::default();

        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: process.clone(),
                    destination: file.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReleaseFlow { destination: file.clone() })
                .await
                .unwrap(),
            SequencerResponse::FlowReleased { source: Some(process), destination: Some(file) }
        );
    }

    #[tokio::test]
    async fn unit_sequencer_layer_flow_interference() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut sequencer = SequencerService::default();

        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: process.clone(),
                    destination: file.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow { source: process, destination: file.clone() })
                .await
                .unwrap_err(),
            TraceabilityError::UnavailableDestination(file)
        );
    }

    #[tokio::test]
    async fn unit_sequencer_layer_flow_circular() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut sequencer = SequencerService::default();

        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: process.clone(),
                    destination: file.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
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
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut sequencer = SequencerService::default();

        let process = Resource::new_process_mock(0);
        let file1 = Resource::new_file("/tmp/test1".to_string());
        let file2 = Resource::new_file("/tmp/test2".to_string());

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: file1.clone(),
                    destination: process.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReleaseFlow { destination: process.clone() })
                .await
                .unwrap(),
            SequencerResponse::FlowReleased {
                source: Some(file1),
                destination: Some(process.clone())
            }
        );

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: file2.clone(),
                    destination: process.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReleaseFlow { destination: process.clone() })
                .await
                .unwrap(),
            SequencerResponse::FlowReleased { source: Some(file2), destination: Some(process) }
        );
    }

    #[tokio::test]
    async fn unit_sequencer_layer_flow_sequence_interference() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut sequencer = SequencerService::default();

        let process1 = Resource::new_process_mock(1);
        let process2 = Resource::new_process_mock(2);
        let file1 = Resource::new_file("/tmp/test1".to_string());
        let file2 = Resource::new_file("/tmp/test2".to_string());

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: file1.clone(),
                    destination: process1.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        // Fails because try get write of write lock
        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
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
                .call(SequencerRequest::ReserveFlow {
                    source: process2,
                    destination: file1.clone(),
                })
                .await
                .unwrap_err(),
            TraceabilityError::UnavailableDestination(file1.clone())
        );

        // Fails because try get read on write lock
        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: process1.clone(),
                    destination: file2,
                })
                .await
                .unwrap_err(),
            TraceabilityError::UnavailableSource(process1.clone())
        );

        // Fails because circular flow (get read on write lock & get write on read lock)
        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: process1.clone(),
                    destination: file1.clone(),
                })
                .await
                .unwrap_err(),
            TraceabilityError::UnavailableSourceAndDestination(process1, file1)
        );
    }
    #[tokio::test]
    async fn unit_sequencer_layer_flow_sequence_interference_multiple_share_releases() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut sequencer = SequencerService::default();

        let process = Resource::new_process_mock(0);
        let file1 = Resource::new_file("/tmp/test1".to_string());
        let file2 = Resource::new_file("/tmp/test2".to_string());
        let file3 = Resource::new_file("/tmp/test3".to_string());

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: process.clone(),
                    destination: file1.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: process.clone(),
                    destination: file2.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: process.clone(),
                    destination: file3.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReleaseFlow { destination: file2.clone() })
                .await
                .unwrap(),
            SequencerResponse::FlowReleased { source: None, destination: Some(file2) }
        );
        assert_eq!(
            sequencer
                .call(SequencerRequest::ReleaseFlow { destination: file3.clone() })
                .await
                .unwrap(),
            SequencerResponse::FlowReleased { source: None, destination: Some(file3) }
        );

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReleaseFlow { destination: file1.clone() })
                .await
                .unwrap(),
            SequencerResponse::FlowReleased { source: Some(process), destination: Some(file1) }
        );
    }

    #[tokio::test]
    async fn unit_waiting_queue_layer_flow_interference() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(1)))
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, None)))
            .service(SequencerService::default());

        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: process.clone(),
                    destination: file.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        assert!(
            sequencer
                .call(SequencerRequest::ReserveFlow { source: process, destination: file })
                .await
                .is_err(),
        );
    }

    #[tokio::test]
    async fn unit_waiting_queue_layer_flow_circular() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(1)))
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, None)))
            .service(SequencerService::default());

        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: process.clone(),
                    destination: file.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        assert!(
            sequencer
                .call(SequencerRequest::ReserveFlow { source: file, destination: process })
                .await
                .is_err(),
        );
    }

    #[tokio::test]
    async fn unit_waiting_queue_layer_writers_interference_resolution() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(10)))
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, Some(1))))
            .service(SequencerService::default());

        let process = Resource::new_process_mock(0);
        let file1 = Resource::new_file("/tmp/test1".to_string());
        let file2 = Resource::new_file("/tmp/test2".to_string());

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: file1.clone(),
                    destination: process.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        let mut sequencer_clone = sequencer.clone();
        let process_clone = process.clone();
        let file2_clone = file2.clone();
        let res = tokio::spawn(async move {
            sequencer_clone
                .call(SequencerRequest::ReserveFlow {
                    source: file2_clone,
                    destination: process_clone,
                })
                .await
        });

        tokio::time::sleep(Duration::from_millis(5)).await;
        assert_eq!(
            sequencer
                .call(SequencerRequest::ReleaseFlow { destination: process.clone() })
                .await
                .unwrap(),
            SequencerResponse::FlowReleased {
                source: Some(file1),
                destination: Some(process.clone())
            }
        );

        assert_eq!(res.await.unwrap().unwrap(), SequencerResponse::FlowReserved);

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReleaseFlow { destination: process.clone() })
                .await
                .unwrap(),
            SequencerResponse::FlowReleased { source: Some(file2), destination: Some(process) }
        );
    }
    #[tokio::test]
    async fn unit_waiting_queue_layer_writer_readers_interference_resolution() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(2)))
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, Some(1))))
            .service(SequencerService::default());

        let process1 = Resource::new_process_mock(0);
        let process2 = Resource::new_process_mock(1);
        let process3 = Resource::new_process_mock(2);
        let process4 = Resource::new_process_mock(3);
        let file = Resource::new_file("/tmp/test".to_string());

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: file.clone(),
                    destination: process1.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: file.clone(),
                    destination: process2.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: file.clone(),
                    destination: process3.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        let mut sequencer_clone = sequencer.clone();
        let file_clone = file.clone();
        let process4_clone = process4.clone();
        let res = tokio::spawn(async move {
            sequencer_clone
                .call(SequencerRequest::ReserveFlow {
                    source: process4_clone,
                    destination: file_clone,
                })
                .await
        });

        tokio::time::sleep(Duration::from_millis(1)).await;
        assert_eq!(
            sequencer
                .call(SequencerRequest::ReleaseFlow { destination: process1.clone() })
                .await
                .unwrap(),
            SequencerResponse::FlowReleased {
                source: None, // here None means that the source is still reserved as reader
                destination: Some(process1)
            }
        );
        assert_eq!(
            sequencer
                .call(SequencerRequest::ReleaseFlow { destination: process2.clone() })
                .await
                .unwrap(),
            SequencerResponse::FlowReleased {
                source: None, // here None means that the source is still reserved as reader
                destination: Some(process2)
            }
        );
        assert_eq!(
            sequencer
                .call(SequencerRequest::ReleaseFlow { destination: process3.clone() })
                .await
                .unwrap(),
            SequencerResponse::FlowReleased {
                // now source is available again
                source: Some(file.clone()),
                destination: Some(process3)
            }
        );

        // the pending flow is automatically reserved
        assert_eq!(res.await.unwrap().unwrap(), SequencerResponse::FlowReserved);

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReleaseFlow { destination: file.clone() })
                .await
                .unwrap(),
            SequencerResponse::FlowReleased { source: Some(process4), destination: Some(file) }
        );
    }

    #[tokio::test]
    async fn unit_waiting_queue_layer_reader_writer_interference_resolution() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut sequencer = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(2)))
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, Some(1))))
            .service(SequencerService::default());

        let process = Resource::new_process_mock(0);
        let file1 = Resource::new_file("/tmp/test1".to_string());
        let file2 = Resource::new_file("/tmp/test2".to_string());

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: file1.clone(),
                    destination: process.clone(),
                })
                .await
                .unwrap(),
            SequencerResponse::FlowReserved
        );

        let mut sequencer_clone = sequencer.clone();
        let process_clone = process.clone();
        let file2_clone = file2.clone();
        let res = tokio::spawn(async move {
            sequencer_clone
                .call(SequencerRequest::ReserveFlow {
                    source: process_clone,
                    destination: file2_clone,
                })
                .await
        });

        tokio::time::sleep(Duration::from_millis(1)).await;
        assert_eq!(
            sequencer
                .call(SequencerRequest::ReleaseFlow { destination: process.clone() })
                .await
                .unwrap(),
            SequencerResponse::FlowReleased {
                source: Some(file1),
                destination: Some(process.clone())
            }
        );

        assert_eq!(res.await.unwrap().unwrap(), SequencerResponse::FlowReserved);

        assert_eq!(
            sequencer
                .call(SequencerRequest::ReleaseFlow { destination: file2.clone() })
                .await
                .unwrap(),
            SequencerResponse::FlowReleased { source: Some(process), destination: Some(file2) }
        );
    }
}
