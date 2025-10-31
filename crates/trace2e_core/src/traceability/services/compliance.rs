//! # Compliance Module
//!
//! This module implements the compliance system for traceability policies in the trace2e framework.
//! It provides policy management and evaluation capabilities to control data flows between resources
//! based on confidentiality, integrity, deletion status, and consent requirements.
//!
//! ## Overview
//!
//! The compliance system enforces policies on resources and evaluates whether data flows between
//! resources are permitted based on these policies. It supports four main policy dimensions:
//!
//! - **Confidentiality**: Controls whether data is public or secret
//! - **Integrity**: Numeric level indicating data trustworthiness (higher = more trusted)
//! - **Deletion**: Tracks deletion status (not deleted, pending deletion, or deleted)
//! - **Consent**: Boolean flag indicating whether the resource owner has given consent for flows
//!
//! ## Policy Evaluation Rules
//!
//! Data flows are permitted only when:
//! 1. Neither source nor destination is deleted or pending deletion
//! 2. Source integrity level >= destination integrity level
//! 3. Secret data cannot flow to public destinations (but public can flow to secret)
//! 4. Both source and destination have consent (when enforced)
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use dashmap::DashMap;
use tokio::task::JoinSet;
use tower::Service;
#[cfg(feature = "trace2e_tracing")]
use tracing::info;

use crate::traceability::{
    api::types::{ComplianceRequest, ComplianceResponse},
    error::TraceabilityError,
    infrastructure::naming::{LocalizedResource, Resource},
    services::consent::{ConsentRequest, ConsentResponse, ConsentService},
};

/// Confidentiality policy defines the level of confidentiality of a resource.
///
/// This enum controls whether data associated with a resource is considered sensitive
/// and restricts flows from secret resources to public destinations.
///
/// # Flow Rules
///
/// - `Public` → `Public`: ✅ Allowed
/// - `Public` → `Secret`: ✅ Allowed (upgrading confidentiality)
/// - `Secret` → `Secret`: ✅ Allowed
/// - `Secret` → `Public`: ❌ Blocked (would leak sensitive data)
#[derive(Default, PartialEq, Debug, Clone, Copy, Eq)]
pub enum ConfidentialityPolicy {
    /// Data that must be kept confidential and cannot flow to public destinations
    Secret,
    /// Data that can be shared publicly (default)
    #[default]
    Public,
}

/// Deletion policy defines the deletion status of a resource.
///
/// This enum tracks the lifecycle state of a resource with respect to deletion,
/// supporting a two-phase deletion process where resources are first marked for
/// deletion and then actually deleted.
///
/// # State Transitions
///
/// ```text
/// NotDeleted → Pending → Deleted
/// ```
///
/// # Flow Rules
///
/// Resources with `Pending` or `Deleted` status cannot be involved in data flows
/// (both as source and destination).
#[derive(Default, PartialEq, Debug, Clone, Eq, Copy)]
pub enum DeletionPolicy {
    /// Resource is active and can participate in flows (default)
    #[default]
    NotDeleted,
    /// Resource is marked for deletion but not yet removed
    Pending,
    /// Resource has been fully deleted
    Deleted,
}

impl From<bool> for DeletionPolicy {
    fn from(deleted: bool) -> Self {
        if deleted { DeletionPolicy::Deleted } else { DeletionPolicy::NotDeleted }
    }
}

/// Policy for a resource that controls compliance checking for data flows.
///
/// A `Policy` combines multiple dimensions of access control and resource management
/// to determine whether data flows involving a resource should be permitted.
///
/// # Fields
///
/// - **`confidentiality`**: Controls whether the resource contains sensitive data
/// - **`integrity`**: Numeric trust level (0 = lowest, higher = more trusted)
/// - **`deleted`**: Tracks deletion status through a multi-phase process
/// - **`consent`**: Whether the resource owner consent is required for flows
///
/// # Policy Evaluation
///
/// When evaluating flows between resources, policies are checked to ensure:
/// 1. No deleted resources are involved
/// 2. Integrity levels are compatible (source >= destination)
/// 3. Confidentiality is preserved (secret data doesn't leak to public)
/// 4. All parties have given consent
///
/// This policy is used to check the compliance of input/output flows of the associated resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Policy {
    /// Confidentiality level of the resource
    confidentiality: ConfidentialityPolicy,
    /// Integrity level (0 = lowest trust, higher values = more trusted)
    integrity: u32,
    /// Deletion status of the resource
    deleted: DeletionPolicy,
    /// Whether the resource owner consent is required for flows
    consent: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 0,
            deleted: DeletionPolicy::NotDeleted,
            consent: false,
        }
    }
}

impl Policy {
    /// Creates a new policy with the specified parameters.
    ///
    /// # Arguments
    ///
    /// * `confidentiality` - The confidentiality level for the resource
    /// * `integrity` - The integrity level (0 = lowest, higher = more trusted)
    /// * `deleted` - The deletion status
    /// * `consent` - Whether the resource owner consent is required for flows
    pub fn new(
        confidentiality: ConfidentialityPolicy,
        integrity: u32,
        deleted: DeletionPolicy,
        consent: bool,
    ) -> Self {
        Self { confidentiality, integrity, deleted, consent }
    }

    /// Returns true if the resource contains confidential data.
    ///
    /// This is a convenience method that checks if the confidentiality policy
    /// is set to `Secret`.
    pub fn is_confidential(&self) -> bool {
        self.confidentiality == ConfidentialityPolicy::Secret
    }

    /// Returns true if the resource is deleted or pending deletion.
    ///
    /// Resources that are deleted cannot participate in data flows.
    pub fn is_deleted(&self) -> bool {
        self.deleted != DeletionPolicy::NotDeleted
    }

    /// Returns true if the resource is pending deletion.
    ///
    /// This indicates the resource has been marked for deletion but hasn't
    /// been fully removed yet.
    pub fn is_pending_deletion(&self) -> bool {
        self.deleted == DeletionPolicy::Pending
    }

    /// Returns the integrity level of the resource.
    ///
    /// Higher values indicate more trusted data. For flows to be permitted,
    /// the source integrity must be greater than or equal to the destination integrity.
    pub fn get_integrity(&self) -> u32 {
        self.integrity
    }

    /// Returns true if the resource owner has given consent for flows.
    ///
    /// When consent is false, flows involving this resource should be denied.
    pub fn get_consent(&self) -> bool {
        self.consent
    }

    /// Updates the consent flag for this policy.
    ///
    /// Returns `PolicyUpdated` if the consent was successfully changed,
    /// or `PolicyNotUpdated` if the resource is deleted and cannot be modified.
    pub fn with_consent(&mut self, consent: bool) -> ComplianceResponse {
        if !self.is_deleted() {
            self.consent = consent;
            ComplianceResponse::PolicyUpdated
        } else {
            ComplianceResponse::PolicyNotUpdated
        }
    }

    /// Updates the integrity level for this policy.
    ///
    /// Returns `PolicyUpdated` if the integrity was successfully changed,
    /// or `PolicyNotUpdated` if the resource is deleted and cannot be modified.
    ///
    /// # Arguments
    ///
    /// * `integrity` - The new integrity level
    pub fn with_integrity(&mut self, integrity: u32) -> ComplianceResponse {
        if !self.is_deleted() {
            self.integrity = integrity;
            ComplianceResponse::PolicyUpdated
        } else {
            ComplianceResponse::PolicyNotUpdated
        }
    }

    /// Updates the confidentiality level for this policy.
    ///
    /// Returns `PolicyUpdated` if the confidentiality was successfully changed,
    /// or `PolicyNotUpdated` if the resource is deleted and cannot be modified.
    ///
    /// # Arguments
    ///
    /// * `confidentiality` - The new confidentiality level
    pub fn with_confidentiality(
        &mut self,
        confidentiality: ConfidentialityPolicy,
    ) -> ComplianceResponse {
        if !self.is_deleted() {
            self.confidentiality = confidentiality;
            ComplianceResponse::PolicyUpdated
        } else {
            ComplianceResponse::PolicyNotUpdated
        }
    }

    /// Marks the resource for deletion.
    ///
    /// This transitions the resource from `NotDeleted` to `Pending` deletion status.
    /// Once marked for deletion, the policy cannot be further modified.
    ///
    /// Returns `PolicyUpdated` if the deletion was successfully marked as pending,
    /// or `PolicyNotUpdated` if the resource is already deleted or pending deletion.
    pub fn deleted(&mut self) -> ComplianceResponse {
        if !self.is_deleted() {
            self.deleted = DeletionPolicy::Pending;
            ComplianceResponse::PolicyUpdated
        } else {
            ComplianceResponse::PolicyNotUpdated
        }
    }

    /// Marks the deletion as enforced for a resource that is pending deletion.
    ///
    /// This transitions the resource from `Pending` to `Deleted` status.
    /// This method should be called after the actual deletion has been performed.
    ///
    /// Returns `PolicyUpdated` if the deletion was successfully marked,
    /// or `PolicyNotUpdated` if the resource is not pending deletion.
    pub fn deletion_enforced(&mut self) -> ComplianceResponse {
        if self.is_pending_deletion() {
            self.deleted = DeletionPolicy::Deleted;
            ComplianceResponse::PolicyUpdated
        } else {
            ComplianceResponse::PolicyNotUpdated
        }
    }
}

/// The main compliance service that manages policies and evaluates flows.
///
/// `ComplianceService` implements the `Service` trait from the Tower library,
/// providing an asynchronous interface for handling compliance requests.
/// It combines policy storage and evaluation logic in a single service.
///
/// # Features
///
/// - **Policy Management**: Store, retrieve, and update resource policies
/// - **Flow Evaluation**: Check whether data flows comply with policies
/// - **Thread Safety**: Safe for concurrent use across multiple threads
/// - **Async Interface**: Non-blocking operations using Tower's Service trait
///
/// # Request Types
///
/// The service handles several types of compliance requests:
///
/// - `EvalPolicies` - Evaluate whether a flow is permitted
/// - `GetPolicy` / `GetPolicies` - Retrieve existing policies
/// - `SetPolicy` - Set complete policy for a resource
/// - `SetConfidentiality` / `SetIntegrity` / `SetConsent` - Update specific policy fields
/// - `SetDeleted` - Mark resources for deletion
///
/// # Operating Modes
///
/// ## Normal Mode (default)
/// - Used in production
/// - Unknown resources get default policies automatically
/// - More forgiving for dynamic resource discovery
///
/// # Error Handling
///
/// The service returns `TraceabilityError` for various failure conditions:
/// - `DirectPolicyViolation` - Flow violates compliance rules
/// - `InternalTrace2eError` - Internal service errors
#[derive(Clone, Debug)]
pub struct ComplianceService<C = ConsentService> {
    /// Node ID
    node_id: String,
    /// Thread-safe storage for resource policies
    policies: Arc<DashMap<Resource, Policy>>,
    /// Consent service
    consent: C,
}

impl Default for ComplianceService {
    fn default() -> Self {
        Self {
            node_id: String::new(),
            policies: Arc::new(DashMap::new()),
            consent: ConsentService::default(),
        }
    }
}

impl ComplianceService<ConsentService> {
    /// Creates a new compliance service with the specified node ID and consent service.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The node ID
    /// * `consent` - The consent service
    ///
    /// # Returns
    ///
    /// A new compliance service.
    pub fn new(node_id: String, consent: ConsentService) -> Self {
        Self { node_id, policies: Arc::new(DashMap::new()), consent }
    }

    /// Evaluates whether a data flow is compliant with the given policies.
    ///
    /// This function implements the core compliance logic by checking multiple policy
    /// dimensions against a set of rules that determine whether data can flow from
    /// source resources to a destination resource.
    ///
    /// # Policy Evaluation Rules
    ///
    /// A flow is **permitted** only when ALL of the following conditions are met:
    ///
    /// 1. **No Deleted Resources**: Neither source nor destination is deleted or pending deletion
    /// 2. **Integrity Preservation**: Source integrity ≥ destination integrity
    /// 3. **Confidentiality Protection**: Secret data cannot flow to public destinations
    /// 4. **Consent Required**: source resources must have consent (when enforced)
    ///
    /// # Arguments
    ///
    /// * `sources` - Set of source resources
    /// * `destination` - Destination resource
    ///
    /// # Returns
    ///
    /// - `Ok(ComplianceResponse::Grant)` if the flow is permitted
    /// - `Err(TraceabilityError::DirectPolicyViolation)` if any rule is violated
    ///
    /// # Flow Scenarios
    ///
    /// ## ✅ Permitted Flows
    /// - Public (integrity 5) → Public (integrity 3) - integrity preserved
    /// - Secret (integrity 5) → Secret (integrity 3) - confidentiality maintained
    /// - Public (integrity 5) → Secret (integrity 3) - upgrading confidentiality
    ///
    /// ## ❌ Blocked Flows  
    /// - Any deleted/pending resource involved
    /// - Public (integrity 3) → Public (integrity 5) - integrity violation
    /// - Secret (integrity 5) → Public (integrity 3) - confidentiality leak
    /// - Any resource without consent (when enforced)
    async fn eval_compliance(
        &self,
        sources: HashSet<Resource>,
        destination: LocalizedResource,
        destination_policy: Option<Policy>,
    ) -> Result<ComplianceResponse, TraceabilityError> {
        // Get the destination policy if it is local or use the provided policy
        let destination_policy = if let Some(destination) = self.as_local_resource(&destination) {
            self.get_policy(&destination)
        } else {
            destination_policy.ok_or(TraceabilityError::DestinationPolicyNotFound)?
        };

        let source_policies = self.get_policies(sources);

        // Collect all consent requests from source policies
        let mut consent_tasks = JoinSet::new();

        for (source, source_policy) in source_policies {
            // Spawn consent request tasks in parallel
            if source_policy.get_consent() {
                let mut consent_service = self.consent.clone();
                let destination = destination.clone().into();
                consent_tasks.spawn(async move {
                    consent_service
                        .call(ConsentRequest::RequestConsent { source, destination })
                        .await
                });
            }

            // If the source or destination policy is deleted, the flow is not compliant
            if source_policy.is_deleted() || destination_policy.is_deleted() {
                #[cfg(feature = "trace2e_tracing")]
                info!(
                    "[compliance] EvalPolicies: source_policy.deleted: {:?}, destination_policy.deleted: {:?}",
                    source_policy.is_deleted(),
                    destination_policy.is_deleted()
                );
                #[cfg(feature = "enforcement_mocking")]
                if source_policy.is_pending_deletion() {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance] Enforcing deletion policy for source");
                }
                #[cfg(feature = "enforcement_mocking")]
                if destination_policy.is_pending_deletion() {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance] Enforcing deletion policy for destination");
                }

                return Err(TraceabilityError::DirectPolicyViolation);
            }

            // Integrity check: Source integrity must be greater than or equal to destination
            // integrity
            if source_policy.integrity < destination_policy.integrity {
                return Err(TraceabilityError::DirectPolicyViolation);
            }

            // Confidentiality check: Secret data cannot flow to public destinations
            if source_policy.confidentiality == ConfidentialityPolicy::Secret
                && destination_policy.confidentiality == ConfidentialityPolicy::Public
            {
                return Err(TraceabilityError::DirectPolicyViolation);
            }
        }

        // Await all consent requests and verify they succeed
        while let Some(result) = consent_tasks.join_next().await {
            let consent_response = result
                .map_err(|_| TraceabilityError::InternalTrace2eError)?
                .map_err(|_| TraceabilityError::InternalTrace2eError)?;

            match consent_response {
                ConsentResponse::Consent(true) => continue,
                ConsentResponse::Consent(false) => {
                    return Err(TraceabilityError::DirectPolicyViolation);
                }
                _ => return Err(TraceabilityError::InternalTrace2eError),
            }
        }

        Ok(ComplianceResponse::Grant)
    }
}

impl ComplianceService {
    /// Returns the resource if it is local to the node, otherwise returns None
    fn as_local_resource(&self, resource: &LocalizedResource) -> Option<Resource> {
        if *resource.node_id() == self.node_id {
            Some(resource.resource().to_owned())
        } else {
            None
        }
    }

    /// Retrieves the policy for a specific resource, inserts a default policy and returns it if not found
    ///
    /// # Arguments
    ///
    /// * `resource` - The resource to look up
    fn get_policy(&self, resource: &Resource) -> Policy {
        self.policies.entry(resource.to_owned()).or_default().to_owned()
    }

    /// Retrieves policies for a set of resources.
    ///
    /// # Arguments
    ///
    /// * `resources` - Set of resources to look up
    ///
    /// # Returns
    ///
    /// A map from resources to their policies, excluding stream resources.
    fn get_policies(&self, resources: HashSet<Resource>) -> HashMap<Resource, Policy> {
        let mut policies_set = HashMap::new();
        for resource in resources {
            // Get the policy from the local policies, streams have no policies
            if !resource.is_stream() {
                policies_set.insert(resource.to_owned(), self.get_policy(&resource));
            }
        }
        policies_set
    }

    /// Retrieves policies for a set of resources.
    ///
    /// # Arguments
    ///
    /// * `resources` - Set of resources to look up
    ///
    /// # Returns
    ///
    /// A map from localized resources to their policies, excluding stream resources.
    fn get_localized_policies(
        &self,
        resources: HashSet<Resource>,
    ) -> HashMap<LocalizedResource, Policy> {
        self.get_policies(resources)
            .into_iter()
            .map(|(resource, policy)| {
                (LocalizedResource::new(self.node_id.clone(), resource), policy)
            })
            .collect()
    }

    /// Sets the complete policy for a specific resource.
    ///
    /// This replaces any existing policy for the resource. If the resource
    /// is already deleted, the update is rejected.
    ///
    /// # Arguments
    ///
    /// * `resource` - The resource to update
    /// * `policy` - The new policy to set
    ///
    /// # Returns
    ///
    /// - `PolicyUpdated` if the policy was successfully set
    /// - `PolicyNotUpdated` if the resource is deleted and cannot be modified
    fn set_policy(&self, resource: Resource, policy: Policy) -> ComplianceResponse {
        // If the resource is not local or is deleted, return PolicyNotUpdated
        if self.policies.get(&resource).is_some_and(|policy| policy.is_deleted()) {
            ComplianceResponse::PolicyNotUpdated
        } else {
            self.policies.insert(resource, policy);
            ComplianceResponse::PolicyUpdated
        }
    }

    /// Sets the confidentiality level for a specific resource.
    ///
    /// Creates a default policy if the resource doesn't exist.
    /// Updates are rejected if the resource is deleted.
    ///
    /// # Arguments
    ///
    /// * `resource` - The resource to update
    /// * `confidentiality` - The new confidentiality level
    fn set_confidentiality(
        &self,
        resource: Resource,
        confidentiality: ConfidentialityPolicy,
    ) -> ComplianceResponse {
        let mut response = ComplianceResponse::PolicyNotUpdated;
        self.policies
            .entry(resource)
            .and_modify(|policy| {
                response = policy.with_confidentiality(confidentiality);
            })
            .or_insert_with(|| {
                let mut policy = Policy::default();
                response = policy.with_confidentiality(confidentiality);
                policy
            });
        response
    }

    /// Sets the integrity level for a specific resource.
    ///
    /// Creates a default policy if the resource doesn't exist.
    /// Updates are rejected if the resource is deleted.
    ///
    /// # Arguments
    ///
    /// * `resource` - The resource to update
    /// * `integrity` - The new integrity level
    fn set_integrity(&self, resource: Resource, integrity: u32) -> ComplianceResponse {
        let mut response = ComplianceResponse::PolicyNotUpdated;
        self.policies
            .entry(resource)
            .and_modify(|policy| {
                response = policy.with_integrity(integrity);
            })
            .or_insert_with(|| {
                let mut policy = Policy::default();
                response = policy.with_integrity(integrity);
                policy
            });
        response
    }

    /// Marks a specific resource for deletion.
    ///
    /// Creates a default policy if the resource doesn't exist, then marks it for deletion.
    /// Updates are rejected if the resource is already deleted.
    ///
    /// # Arguments
    ///
    /// * `resource` - The resource to mark for deletion
    fn set_deleted(&self, resource: Resource) -> ComplianceResponse {
        let mut response = ComplianceResponse::PolicyNotUpdated;
        self.policies
            .entry(resource)
            .and_modify(|policy| {
                response = policy.deleted();
            })
            .or_insert_with(|| {
                let mut policy = Policy::default();
                response = policy.deleted();
                policy
            });
        response
    }

    /// Sets the consent enforcement flag for a specific resource.
    ///
    /// Creates a default policy if the resource doesn't exist.
    /// Updates are rejected if the resource is deleted.
    ///
    /// # Arguments
    ///
    /// * `resource` - The resource to update
    /// * `consent` - The new consent enforcement value
    fn enforce_consent(&self, resource: Resource, consent: bool) -> ComplianceResponse {
        let mut response = ComplianceResponse::PolicyNotUpdated;
        self.policies
            .entry(resource)
            .and_modify(|policy| {
                response = policy.with_consent(consent);
            })
            .or_insert_with(|| {
                let mut policy = Policy::default();
                response = policy.with_consent(consent);
                policy
            });
        response
    }
}

impl Service<ComplianceRequest> for ComplianceService<ConsentService> {
    type Response = ComplianceResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: ComplianceRequest) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            match request {
                ComplianceRequest::EvalCompliance { sources, destination, destination_policy } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance-{}] CheckCompliance: sources: {:?}, destination: {:?}, destination_policy: {:?}",
                        this.node_id, sources, destination, destination_policy
                    );
                    this.eval_compliance(sources, destination, destination_policy).await
                }
                ComplianceRequest::GetPolicy(resource) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance-{}] GetPolicy: resource: {:?}", this.node_id, resource);
                    Ok(ComplianceResponse::Policy(this.get_policy(&resource)))
                }
                ComplianceRequest::GetPolicies(resources) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance-{}] GetPolicies: resources: {:?}", this.node_id, resources);
                    Ok(ComplianceResponse::Policies(this.get_localized_policies(resources)))
                }
                ComplianceRequest::SetPolicy { resource, policy } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance-{}] SetPolicy: resource: {:?}, policy: {:?}",
                        this.node_id, resource, policy
                    );
                    Ok(this.set_policy(resource, policy))
                }
                ComplianceRequest::SetConfidentiality { resource, confidentiality } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance-{}] SetConfidentiality: resource: {:?}, confidentiality: {:?}",
                        this.node_id, resource, confidentiality
                    );
                    Ok(this.set_confidentiality(resource, confidentiality))
                }
                ComplianceRequest::SetIntegrity { resource, integrity } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance-{}] SetIntegrity: resource: {:?}, integrity: {:?}",
                        this.node_id, resource, integrity
                    );
                    Ok(this.set_integrity(resource, integrity))
                }
                ComplianceRequest::SetDeleted(resource) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance-{}] SetDeleted: resource: {:?}", this.node_id, resource);
                    Ok(this.set_deleted(resource))
                }
                ComplianceRequest::EnforceConsent { resource, consent } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance-{}] EnforceConsent: resource: {:?}, consent: {:?}",
                        this.node_id, resource, consent
                    );
                    Ok(this.enforce_consent(resource, consent))
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traceability::infrastructure::naming::Resource;

    // Helper functions to reduce test code duplication
    fn create_public_policy(integrity: u32) -> Policy {
        Policy::new(ConfidentialityPolicy::Public, integrity, DeletionPolicy::NotDeleted, false)
    }

    fn create_secret_policy(integrity: u32) -> Policy {
        Policy::new(ConfidentialityPolicy::Secret, integrity, DeletionPolicy::NotDeleted, false)
    }

    fn create_deleted_policy() -> Policy {
        Policy::new(ConfidentialityPolicy::Public, 0, DeletionPolicy::Pending, false)
    }

    fn init_tracing() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
    }

    #[tokio::test]
    async fn unit_compliance_set_policy_basic() {
        init_tracing();
        let compliance = ComplianceService::default();
        let process = Resource::new_process_mock(0);

        let policy = create_secret_policy(5);

        // First time setting policy should return PolicyUpdated
        assert_eq!(
            compliance.set_policy(process.clone(), policy),
            ComplianceResponse::PolicyUpdated
        );

        let new_policy = create_secret_policy(3);

        // Updating existing policy should also return PolicyUpdated
        assert_eq!(compliance.set_policy(process, new_policy), ComplianceResponse::PolicyUpdated);
    }

    #[tokio::test]
    async fn unit_compliance_policy_evaluation_scenarios() {
        init_tracing();
        let compliance = ComplianceService::default();
        // Test integrity constraints
        let test_cases = [
            // (source_policy, dest_policy, should_pass, description)
            (create_public_policy(5), create_public_policy(3), true, "integrity pass: 5 >= 3"),
            (create_public_policy(3), create_public_policy(5), false, "integrity fail: 3 < 5"),
            (
                create_secret_policy(5),
                create_secret_policy(3),
                true,
                "confidentiality pass: secret -> secret",
            ),
            (
                create_secret_policy(5),
                create_public_policy(3),
                false,
                "confidentiality fail: secret -> public",
            ),
            (
                create_public_policy(5),
                create_secret_policy(3),
                true,
                "confidentiality pass: public -> secret",
            ),
        ];
        let mock_file =
            LocalizedResource::new(String::new(), Resource::new_file("/tmp/dest".to_string()));
        let mock_process = Resource::new_process_mock(0);

        for (source_policy, dest_policy, should_pass, description) in test_cases {
            compliance.set_policy(mock_process.clone(), source_policy);
            compliance.set_policy(mock_file.resource().to_owned(), dest_policy);
            let result = compliance
                .eval_compliance(HashSet::from([mock_process.clone()]), mock_file.clone(), None)
                .await;

            if should_pass {
                assert!(
                    result.is_ok_and(|r| r == ComplianceResponse::Grant),
                    "Test failed: {description}"
                );
            } else {
                assert!(
                    result.is_err_and(|e| e == TraceabilityError::DirectPolicyViolation),
                    "Test failed: {description}"
                );
            }
        }
    }

    #[tokio::test]
    async fn unit_compliance_default_policies() {
        init_tracing();
        let compliance = ComplianceService::default();
        let mock_local_file =
            LocalizedResource::new(String::new(), Resource::new_file("/tmp/dest".to_string()));
        let mock_process = Resource::new_process_mock(0);
        let mock_process2 = Resource::new_process_mock(1);

        // Test 1: All default local policies should pass
        assert!(
            compliance
                .eval_compliance(
                    HashSet::from([mock_process.clone(), mock_process2.clone()]),
                    mock_local_file.clone(),
                    None,
                )
                .await
                .is_ok_and(|r| r == ComplianceResponse::Grant)
        );

        // Test 2: Mixed default and explicit policies - integrity violation
        let dest_policy = create_public_policy(2);
        compliance.set_policy(mock_local_file.resource().to_owned(), dest_policy);
        assert!(
            compliance
                .eval_compliance(
                    HashSet::from([mock_process.clone(), mock_process2.clone()]),
                    mock_local_file, // integrity: 2, so 0 < 2 should fail
                    None,
                )
                .await
                .is_err_and(|e| e == TraceabilityError::DirectPolicyViolation)
        );
    }

    #[tokio::test]
    async fn unit_compliance_service_eval_policies_requests() {
        init_tracing();
        let mut compliance = ComplianceService::default();

        let mock_process = Resource::new_process_mock(0);
        let file =
            LocalizedResource::new(String::new(), Resource::new_file("/tmp/dest".to_string()));

        // Test case 1: Valid policy flow - should grant (process integrity 5 >= file integrity 3)
        let process_policy = create_public_policy(5);
        compliance.set_policy(mock_process.clone(), process_policy);
        let file_policy = create_public_policy(3);
        compliance.set_policy(file.resource().to_owned(), file_policy);

        let grant_request = ComplianceRequest::EvalCompliance {
            sources: HashSet::from([mock_process.clone()]),
            destination: file.clone(),
            destination_policy: None,
        };
        assert_eq!(compliance.call(grant_request).await.unwrap(), ComplianceResponse::Grant);

        // Test case 2: Invalid policy flow - should deny (process integrity 2 < file integrity 3)
        let low_process_policy = create_public_policy(2);
        compliance.set_policy(mock_process.clone(), low_process_policy);

        let deny_request = ComplianceRequest::EvalCompliance {
            sources: HashSet::from([mock_process.clone()]),
            destination: file,
            destination_policy: None,
        };
        assert_eq!(
            compliance.call(deny_request).await.unwrap_err(),
            TraceabilityError::DirectPolicyViolation
        );
    }

    #[test]
    fn unit_compliance_get_policies_empty() {
        init_tracing();
        let compliance = ComplianceService::default();
        let process = LocalizedResource::new(String::new(), Resource::new_process_mock(0));
        let file =
            LocalizedResource::new(String::new(), Resource::new_file("/tmp/test".to_string()));

        assert_eq!(
            compliance.get_localized_policies(HashSet::from([
                process.resource().to_owned(),
                file.resource().to_owned()
            ])),
            HashMap::from([(process, Policy::default()), (file, Policy::default())])
        );
    }

    #[test]
    fn unit_compliance_get_policies_scenarios() {
        init_tracing();
        let compliance = ComplianceService::default();

        // Test resources
        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());

        // Test policies
        let process_policy = create_secret_policy(7);
        let file_policy = create_public_policy(3);

        // Test 1: Single resource with policy
        compliance.set_policy(process.clone(), process_policy.clone());
        assert_eq!(
            compliance.get_policies(HashSet::from([process.clone()])),
            HashMap::from([(process.clone(), process_policy.clone())])
        );

        // Test 2: Multiple resources with policies
        compliance.set_policy(file.clone(), file_policy.clone());
        assert_eq!(
            compliance.get_policies(HashSet::from([process.clone(), file.clone()])),
            HashMap::from([
                (process.clone(), process_policy.clone()),
                (file.clone(), file_policy.clone())
            ])
        );

        // Test 3: Mixed existing and default policies
        let new_file = Resource::new_file("/tmp/new.txt".to_string());
        assert_eq!(
            compliance.get_policies(HashSet::from([
                process.clone(),
                file.clone(),
                new_file.clone()
            ])),
            HashMap::from([
                (process.clone(), process_policy.clone()),
                (file.clone(), file_policy.clone()),
                (new_file.clone(), Policy::default())
            ])
        );
    }

    #[tokio::test]
    async fn unit_compliance_deleted_policy_behavior() {
        init_tracing();
        let compliance = ComplianceService::default();
        let mock_process0 = Resource::new_process_mock(0);
        let mock_process1 = Resource::new_process_mock(1);
        let mock_file = Resource::new_file("/tmp/dest".to_string());

        // Create and set a deleted policy
        let deleted_policy = create_deleted_policy();
        compliance.set_policy(mock_process0.clone(), deleted_policy.clone());

        // Test 1: Deleted policy is returned correctly
        assert_eq!(
            compliance.get_policies(HashSet::from([mock_process0.clone()])),
            HashMap::from([(mock_process0.clone(), deleted_policy.clone())])
        );

        // Test 2: Cannot update deleted policy
        assert_eq!(
            compliance.set_policy(mock_process0.clone(), Policy::default()),
            ComplianceResponse::PolicyNotUpdated
        );

        // Test 3: Policy remains deleted after update attempt
        assert_eq!(
            compliance.get_policies(HashSet::from([mock_process0.clone()])),
            HashMap::from([(mock_process0.clone(), deleted_policy.clone())])
        );

        // Test 4: Deleted policies cause policy violations in evaluation in both directions
        assert!(
            compliance
                .eval_compliance(
                    HashSet::from([mock_process0.clone(), mock_process1.clone()]),
                    LocalizedResource::new(String::new(), mock_file.clone()),
                    None
                )
                .await
                .is_err_and(|e| e == TraceabilityError::DirectPolicyViolation)
        );

        assert!(
            compliance
                .eval_compliance(
                    HashSet::from([mock_file.clone(), mock_process1.clone()]),
                    LocalizedResource::new(String::new(), mock_process0),
                    None
                )
                .await
                .is_err_and(|e| e == TraceabilityError::DirectPolicyViolation)
        );
    }

    #[tokio::test]
    async fn unit_compliance_localized_destination() {
        init_tracing();
        let compliance = ComplianceService::default();

        let local_process = Resource::new_process_mock(0);
        let local_file = Resource::new_file("/tmp/local.txt".to_string());
        let remote_process =
            LocalizedResource::new("10.0.0.1".to_string(), Resource::new_process_mock(1));

        // Test 2: Non-local destination without policy should fail
        assert!(
            compliance
                .eval_compliance(
                    HashSet::from([local_file.clone(), local_process.clone()]),
                    remote_process.clone(),
                    None
                )
                .await
                .is_err_and(|e| e == TraceabilityError::DestinationPolicyNotFound)
        );

        // Test 3: Non-local destination WITH policy should succeed (the exception)
        let remote_policy = create_public_policy(0);
        assert!(
            compliance
                .eval_compliance(
                    HashSet::from([local_file, local_process]),
                    remote_process.clone(),
                    Some(remote_policy.clone())
                )
                .await
                .is_ok_and(|r| r == ComplianceResponse::Grant)
        );
    }

    #[tokio::test]
    async fn unit_compliance_service_deletion_policy_workflow() {
        init_tracing();
        let mut compliance = ComplianceService::default();

        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/sensitive.txt".to_string());

        // Test 1: Set initial policies for resources
        let process_policy = create_secret_policy(5);
        let file_policy = create_public_policy(3);

        let set_process_request = ComplianceRequest::SetPolicy {
            resource: process.clone(),
            policy: process_policy.clone(),
        };
        assert_eq!(
            compliance.call(set_process_request).await.unwrap(),
            ComplianceResponse::PolicyUpdated
        );

        let set_file_request =
            ComplianceRequest::SetPolicy { resource: file.clone(), policy: file_policy.clone() };
        assert_eq!(
            compliance.call(set_file_request).await.unwrap(),
            ComplianceResponse::PolicyUpdated
        );

        // Test 2: Verify policies are set correctly
        let get_process_request = ComplianceRequest::GetPolicy(process.clone());
        if let ComplianceResponse::Policy(retrieved_policy) =
            compliance.call(get_process_request).await.unwrap()
        {
            assert_eq!(retrieved_policy, process_policy);
            assert!(!retrieved_policy.is_deleted());
        } else {
            panic!("Expected Policy response");
        }

        // Test 3: Create a policy with pending deletion status
        let mut pending_deletion_policy = create_secret_policy(7);
        pending_deletion_policy.deleted();
        assert!(pending_deletion_policy.is_pending_deletion());

        let set_pending_request = ComplianceRequest::SetPolicy {
            resource: file.clone(),
            policy: pending_deletion_policy.clone(),
        };
        assert_eq!(
            compliance.call(set_pending_request).await.unwrap(),
            ComplianceResponse::PolicyUpdated
        );

        // Test 4: Verify pending deletion policy is set
        let get_file_request = ComplianceRequest::GetPolicy(file.clone());
        if let ComplianceResponse::Policy(retrieved_policy) =
            compliance.call(get_file_request).await.unwrap()
        {
            assert_eq!(retrieved_policy, pending_deletion_policy);
            assert!(retrieved_policy.is_deleted());
            assert!(retrieved_policy.is_pending_deletion());
        } else {
            panic!("Expected Policy response");
        }

        // Test 5: Try to update policy on resource with pending deletion - should fail
        let new_policy = create_public_policy(1);
        let update_pending_request =
            ComplianceRequest::SetPolicy { resource: file.clone(), policy: new_policy };
        assert_eq!(
            compliance.call(update_pending_request).await.unwrap(),
            ComplianceResponse::PolicyNotUpdated
        );

        // Test 6: Create fully deleted policy and test it
        let mut deleted_policy = create_public_policy(2);
        deleted_policy.deleted();
        deleted_policy.deletion_enforced();
        assert!(deleted_policy.is_deleted());
        assert!(!deleted_policy.is_pending_deletion());

        let new_resource = Resource::new_process_mock(1);
        let set_deleted_request = ComplianceRequest::SetPolicy {
            resource: new_resource.clone(),
            policy: deleted_policy.clone(),
        };
        assert_eq!(
            compliance.call(set_deleted_request).await.unwrap(),
            ComplianceResponse::PolicyUpdated
        );

        // Test 7: Verify deleted policy blocks updates
        let another_policy = create_secret_policy(10);
        let update_deleted_request =
            ComplianceRequest::SetPolicy { resource: new_resource.clone(), policy: another_policy };
        assert_eq!(
            compliance.call(update_deleted_request).await.unwrap(),
            ComplianceResponse::PolicyNotUpdated
        );

        // Test 8: Test policy evaluation with deleted resources - should fail
        let eval_with_deleted_source = ComplianceRequest::EvalCompliance {
            sources: HashSet::from([new_resource]),
            destination: LocalizedResource::new(
                String::new(),
                Resource::new_file("/tmp/dest".to_string()),
            ),
            destination_policy: None,
        };
        assert_eq!(
            compliance.call(eval_with_deleted_source).await.unwrap_err(),
            TraceabilityError::DirectPolicyViolation
        );

        // Test 9: Test policy evaluation with deleted destination - should fail
        let eval_with_deleted_dest = ComplianceRequest::EvalCompliance {
            sources: HashSet::from([process]),
            destination: LocalizedResource::new(String::new(), file),
            destination_policy: None,
        };
        assert_eq!(
            compliance.call(eval_with_deleted_dest).await.unwrap_err(),
            TraceabilityError::DirectPolicyViolation
        );
    }
}
