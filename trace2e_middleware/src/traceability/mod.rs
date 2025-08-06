//! Traceability module.
pub mod api;
pub mod core;
pub mod error;
pub mod m2m;
pub mod naming;
pub mod o2m;
pub mod p2m;
pub mod validation;

pub type M2mApiDefaultStack = m2m::M2mApiService<
    core::sequencer::WaitingQueueService<core::sequencer::SequencerService>,
    core::provenance::ProvenanceService,
    core::compliance::ComplianceService,
>;

pub type P2mApiDefaultStack<M> = p2m::P2mApiService<
    core::sequencer::WaitingQueueService<core::sequencer::SequencerService>,
    core::provenance::ProvenanceService,
    core::compliance::ComplianceService,
    M,
>;

pub type O2mApiDefaultStack =
    o2m::O2mApiService<core::provenance::ProvenanceService, core::compliance::ComplianceService>;

/// Helper function to initialize the middleware stack.
///
/// This function initializes the middleware stack with the given max_retries and m2m_client.
pub fn init_middleware<M>(
    max_retries: Option<u32>,
    m2m_client: M,
) -> (
    M2mApiDefaultStack,
    P2mApiDefaultStack<M>,
    O2mApiDefaultStack,
)
where
    M: tower::Service<
            api::M2mRequest,
            Response = api::M2mResponse,
            Error = error::TraceabilityError,
        > + Clone
        + Send
        + 'static,
    M::Future: Send,
{
    let sequencer = tower::ServiceBuilder::new()
        .layer(tower::layer::layer_fn(|inner| {
            core::sequencer::WaitingQueueService::new(inner, max_retries)
        }))
        .service(core::sequencer::SequencerService::default());
    let provenance = core::provenance::ProvenanceService::default();
    let compliance = core::compliance::ComplianceService::default();

    let m2m_service: M2mApiDefaultStack =
        m2m::M2mApiService::new(sequencer.clone(), provenance.clone(), compliance.clone());

    let p2m_service: P2mApiDefaultStack<M> = p2m::P2mApiService::new(
        sequencer,
        provenance.clone(),
        compliance.clone(),
        m2m_client,
    );

    let o2m_service: O2mApiDefaultStack = o2m::O2mApiService::new(provenance, compliance);

    (m2m_service, p2m_service, o2m_service)
}
