use tokio::sync::oneshot;

use crate::{identifier::Identifier, labels::ComplianceLabel};

use super::TraceabilityError;

/// Provenance layer response message type.
#[derive(Debug)]
pub enum TraceabilityResponse {
    Registered(Identifier),
    Declared(u64),
    Recorded,
    Error(TraceabilityError),
    WaitingSync(oneshot::Sender<(Vec<ComplianceLabel>, oneshot::Sender<TraceabilityResponse>)>),
}

impl TraceabilityResponse {
    /// Returns `true` if the provenance result is [`WaitingSync`].
    ///
    /// [`WaitingSync`]: TraceabilityResponse::WaitingSync
    #[must_use]
    pub fn is_waiting_sync(&self) -> bool {
        matches!(self, Self::WaitingSync(..))
    }
}

/// Provenance layer request message type.
pub enum TraceabilityRequest {
    RegisterContainer(Identifier, oneshot::Sender<TraceabilityResponse>),
    SetComplianceLabel(
        Identifier,
        Option<bool>,
        Option<bool>,
        oneshot::Sender<TraceabilityResponse>,
    ),
    DeclareFlow(
        Identifier,
        Identifier,
        bool,
        oneshot::Sender<TraceabilityResponse>,
    ),
    RecordFlow(u64, oneshot::Sender<TraceabilityResponse>),
    SyncStream(Identifier, oneshot::Sender<TraceabilityResponse>),
    PrintProvenance,
}
