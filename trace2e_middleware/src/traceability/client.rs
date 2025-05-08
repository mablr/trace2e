use tokio::sync::{mpsc, oneshot};
use crate::{
    identifier::Identifier,
    labels::{ComplianceLabel, Labels},
};

use super::{TraceabilityRequest, TraceabilityResponse, TraceabilityError};

/// Client for interacting with the traceability server
#[derive(Clone)]
pub struct TraceabilityClient {
    sender: mpsc::Sender<TraceabilityRequest>,
}


impl TraceabilityClient {
    /// Create a new traceability client with the given sender
    pub fn new(sender: mpsc::Sender<TraceabilityRequest>) -> Self {
        Self { sender }
    }

    /// Register a container with the traceability server
    pub async fn register_container(&self, identifier: Identifier) -> Result<Identifier, TraceabilityError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(TraceabilityRequest::RegisterContainer(identifier, response_tx))
            .await
            .map_err(|_| TraceabilityError::ChannelError)?;

        match response_rx.await {
            Ok(TraceabilityResponse::Registered(id)) => Ok(id),
            Ok(TraceabilityResponse::Error(e)) => Err(e),
            _ => Err(TraceabilityError::InvalidResponse),
        }
    }

    /// Set compliance label for a container
    pub async fn set_compliance_label(
        &self,
        identifier: Identifier,
        local_confidentiality: Option<bool>,
        local_integrity: Option<bool>,
    ) -> Result<(), TraceabilityError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(TraceabilityRequest::SetComplianceLabel(
                identifier,
                local_confidentiality,
                local_integrity,
                response_tx,
            ))
            .await
            .map_err(|_| TraceabilityError::ChannelError)?;

        match response_rx.await {
            Ok(TraceabilityResponse::Recorded) => Ok(()),
            Ok(TraceabilityResponse::Error(e)) => Err(e),
            _ => Err(TraceabilityError::InvalidResponse),
        }
    }

    /// Declare a flow between containers
    pub async fn declare_flow(
        &self,
        source: Identifier,
        destination: Identifier,
        output: bool,
    ) -> Result<u64, TraceabilityError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(TraceabilityRequest::DeclareFlow(
                source,
                destination,
                output,
                response_tx,
            ))
            .await
            .map_err(|_| TraceabilityError::ChannelError)?;

        match response_rx.await {
            Ok(TraceabilityResponse::Declared(flow_id)) => Ok(flow_id),
            Ok(TraceabilityResponse::Error(e)) => Err(e),
            _ => Err(TraceabilityError::InvalidResponse),
        }
    }

    /// Record a flow
    pub async fn record_flow(&self, flow_id: u64) -> Result<(), TraceabilityError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(TraceabilityRequest::RecordFlow(flow_id, response_tx))
            .await
            .map_err(|_| TraceabilityError::ChannelError)?;

        match response_rx.await {
            Ok(TraceabilityResponse::Recorded) => Ok(()),
            Ok(TraceabilityResponse::Error(e)) => Err(e),
            _ => Err(TraceabilityError::InvalidResponse),
        }
    }

    /// Sync stream for a container
    pub async fn sync_stream(&self, identifier: Identifier) -> Result<(Labels, oneshot::Sender<(Vec<ComplianceLabel>, oneshot::Sender<TraceabilityResponse>)>), TraceabilityError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(TraceabilityRequest::SyncStream(identifier, response_tx))
            .await
            .map_err(|_| TraceabilityError::ChannelError)?;

        match response_rx.await {
            Ok(TraceabilityResponse::WaitingSync(labels, provenance_sync_channel)) => {
                Ok((labels, provenance_sync_channel))
            }
            Ok(TraceabilityResponse::Error(e)) => Err(e),
            _ => Err(TraceabilityError::InvalidResponse),
        }
    }

    /// Print provenance information
    pub async fn print_provenance(&self) -> Result<(), TraceabilityError> {
        self.sender
            .send(TraceabilityRequest::PrintProvenance)
            .await
            .map_err(|_| TraceabilityError::ChannelError)?;
        Ok(())
    }
}
