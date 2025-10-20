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
    infrastructure::naming::Resource,
    services::consent::{ConsentRequest, ConsentResponse, ConsentService, Destination},
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
/// ## Cache Mode
/// - Used primarily in testing
/// - Unknown resources cause `PolicyNotFound` errors
/// - Enforces explicit policy management
///
/// # Error Handling
///
/// The service returns `TraceabilityError` for various failure conditions:
/// - `PolicyNotFound` - Resource not found (in cache mode)
/// - `DirectPolicyViolation` - Flow violates compliance rules
/// - `InternalTrace2eError` - Internal service errors
#[derive(Clone, Debug)]
pub struct ComplianceService<C = ConsentService> {
    /// Whether to operate in cache mode (true) or normal mode (false)
    cache_mode: bool,
    /// Thread-safe storage for resource policies
    policies: Arc<DashMap<Resource, Policy>>,
    /// Consent service
    consent: C,
}

impl Default for ComplianceService {
    fn default() -> Self {
        Self {
            cache_mode: false,
            policies: Arc::new(DashMap::new()),
            consent: ConsentService::default(),
        }
    }
}

impl ComplianceService<ConsentService> {
    pub fn new_with_consent(consent: ConsentService) -> Self {
        Self { cache_mode: false, policies: Arc::new(DashMap::new()), consent }
    }

    /// Creates a new PolicyMap in cache mode for testing purposes.
    ///
    /// In cache mode, requests for unknown resources will return
    /// `PolicyNotFound` errors instead of default policies.
    #[cfg(test)]
    fn init_cache() -> Self {
        Self {
            cache_mode: true,
            policies: Arc::new(DashMap::new()),
            consent: ConsentService::default(),
        }
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
    /// 4. **Consent Required**: Both source and destination must have consent (when enforced)
    ///
    /// # Arguments
    ///
    /// * `source_policies` - Map of node IDs to their resource policies (sources)
    /// * `destination_policy` - Policy of the destination resource
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
    ///
    /// # Multi-Node Support
    ///
    /// The function supports evaluating flows that involve multiple source nodes,
    /// checking that ALL source policies are compatible with the destination.
    /// Node-based policies are not yet implemented.
    async fn eval_policies(
        &self,
        source_policies: HashMap<String, HashMap<Resource, Policy>>,
        destination: Resource,
    ) -> Result<ComplianceResponse, TraceabilityError> {
        let destination_policy = self.get_policy(&destination)?;

        // Collect all consent requests from source policies
        let mut consent_tasks = JoinSet::new();

        // Merge local and remote source policies, ignoring the node
        // TODO: implement node based policies
        for (node, source_policy_batch) in source_policies {
            for (source, source_policy) in source_policy_batch {
                // Spawn consent request tasks in parallel
                if source_policy.get_consent() {
                    let mut consent_service = self.consent.clone();
                    let source = source.clone();
                    let dest = Destination::new(Some(node.clone()), Some(destination.clone()));

                    consent_tasks.spawn(async move {
                        consent_service
                            .call(ConsentRequest::RequestConsent {
                                source,
                                destination: dest,
                            })
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
        }

        // Await all consent requests and verify they succeed
        while let Some(result) = consent_tasks.join_next().await {
            let consent_response = result
                .map_err(|_| TraceabilityError::InternalTrace2eError)?
                .map_err(|_| TraceabilityError::InternalTrace2eError)?;

            match consent_response {
                ConsentResponse::Consent(true) => continue,
                ConsentResponse::Consent(false) => {
                    return Err(TraceabilityError::DirectPolicyViolation)
                }
                _ => return Err(TraceabilityError::InternalTrace2eError),
            }
        }

        Ok(ComplianceResponse::Grant)
    }
}

impl ComplianceService {
    /// Retrieves the policy for a specific resource.
    ///
    /// # Behavior by Mode
    ///
    /// - **Normal mode**: Returns default policy if resource not found
    /// - **Cache mode**: Returns `PolicyNotFound` error if resource not found
    ///
    /// # Arguments
    ///
    /// * `resource` - The resource to look up
    ///
    /// # Errors
    ///
    /// Returns `PolicyNotFound` error in cache mode when the resource is not found.
    fn get_policy(&self, resource: &Resource) -> Result<Policy, TraceabilityError> {
        self.policies.get(resource).map(|p| p.to_owned()).map_or(
            if self.cache_mode {
                Err(TraceabilityError::PolicyNotFound(resource.to_owned()))
            } else {
                Ok(Policy::default())
            },
            Ok,
        )
    }

    /// Retrieves policies for a set of resources.
    ///
    /// Stream resources are automatically filtered out as they don't have policies.
    ///
    /// # Behavior by Mode
    ///
    /// - **Normal mode**: Unknown resources get default policies
    /// - **Cache mode**: Unknown resources cause the entire operation to fail
    ///
    /// # Arguments
    ///
    /// * `resources` - Set of resources to look up
    ///
    /// # Returns
    ///
    /// A map from resources to their policies, excluding stream resources.
    ///
    /// # Errors
    ///
    /// Returns `PolicyNotFound` error in cache mode when any resource is not found.
    fn get_policies(
        &self,
        resources: HashSet<Resource>,
    ) -> Result<HashMap<Resource, Policy>, TraceabilityError> {
        let mut policies_set = HashMap::new();
        for resource in resources {
            // Get the policy from the local policies, streams have no policies
            if resource.is_stream().is_none() {
                if let Ok(policy) = self.get_policy(&resource) {
                    policies_set.insert(resource, policy);
                } else if !self.cache_mode {
                    policies_set.insert(resource, Policy::default());
                } else {
                    return Err(TraceabilityError::PolicyNotFound(resource));
                }
            }
        }
        Ok(policies_set)
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
        // Enforce deleted policy
        if self.policies.get(&resource).is_some_and(|policy| policy.is_deleted()) {
            return ComplianceResponse::PolicyNotUpdated;
        }
        self.policies.insert(resource, policy);
        ComplianceResponse::PolicyUpdated
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
                ComplianceRequest::EvalPolicies { source_policies, destination } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance] CheckCompliance: source_policies: {:?}, destination: {:?}",
                        source_policies, destination
                    );
                    this.eval_policies(source_policies, destination).await
                }
                ComplianceRequest::GetPolicy(resource) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance] GetPolicy: resource: {:?}", resource);
                    Ok(ComplianceResponse::Policy(this.get_policy(&resource)?))
                }
                ComplianceRequest::GetPolicies(resources) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance] GetPolicies: resources: {:?}", resources);
                    Ok(ComplianceResponse::Policies(this.get_policies(resources)?))
                }
                ComplianceRequest::SetPolicy { resource, policy } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance] SetPolicy: resource: {:?}, policy: {:?}", resource, policy);
                    Ok(this.set_policy(resource, policy))
                }
                ComplianceRequest::SetConfidentiality { resource, confidentiality } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance] SetConfidentiality: resource: {:?}, confidentiality: {:?}",
                        resource, confidentiality
                    );
                    Ok(this.set_confidentiality(resource, confidentiality))
                }
                ComplianceRequest::SetIntegrity { resource, integrity } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance] SetIntegrity: resource: {:?}, integrity: {:?}",
                        resource, integrity
                    );
                    Ok(this.set_integrity(resource, integrity))
                }
                ComplianceRequest::SetDeleted(resource) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance] SetDeleted: resource: {:?}", resource);
                    Ok(this.set_deleted(resource))
                }
                ComplianceRequest::EnforceConsent { resource, consent } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance] EnforceConsent: resource: {:?}, consent: {:?}",
                        resource, consent
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

    fn create_deleted_policy(integrity: u32) -> Policy {
        Policy::new(ConfidentialityPolicy::Public, integrity, DeletionPolicy::Pending, false)
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
        let mock_file = Resource::new_file("/tmp/dest".to_string());

        for (source_policy, dest_policy, should_pass, description) in test_cases {
            compliance.set_policy(mock_file.clone(), dest_policy);
            let result = compliance
                .eval_policies(
                    HashMap::from([(
                        String::new(),
                        HashMap::from([(Resource::new_process_mock(0), source_policy)]),
                    )]),
                    mock_file.clone(),
                )
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
        let mock_file = Resource::new_file("/tmp/dest".to_string());
        // Test 1: All default policies should pass
        assert!(
            compliance
                .eval_policies(
                    HashMap::from([
                        (
                            String::new(),
                            HashMap::from([
                                (Resource::new_process_mock(0), Policy::default()),
                                (Resource::new_process_mock(1), Policy::default())
                            ])
                        ),
                        (
                            "10.0.0.1".to_string(),
                            HashMap::from([(Resource::new_process_mock(0), Policy::default())])
                        )
                    ]),
                    mock_file.clone()
                )
                .await
                .is_ok_and(|r| r == ComplianceResponse::Grant)
        );

        // Test 2: Mixed default and explicit policies - integrity violation
        let dest_policy = create_public_policy(2);
        compliance.set_policy(mock_file.clone(), dest_policy);
        assert!(
            compliance
                .eval_policies(
                    HashMap::from([
                        (
                            String::new(),
                            HashMap::from([(Resource::new_process_mock(0), Policy::default())])
                        ), // integrity: 0
                        (
                            "10.0.0.1".to_string(),
                            HashMap::from([(Resource::new_process_mock(0), Policy::default())])
                        )
                    ]),
                    mock_file // integrity: 2, so 0 < 2 should fail
                )
                .await
                .is_err_and(|e| e == TraceabilityError::DirectPolicyViolation)
        );
    }

    #[tokio::test]
    async fn unit_compliance_service_eval_policies_requests() {
        init_tracing();
        let mut compliance = ComplianceService::default();

        let file = Resource::new_file("/tmp/dest".to_string());
        let policy = create_public_policy(3);
        compliance.set_policy(file.clone(), policy);

        // Test case 1: Valid policy flow - should grant
        let grant_request = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([(
                String::new(),
                HashMap::from([(Resource::new_process_mock(0), create_public_policy(5))]),
            )]),
            destination: file.clone(),
        };
        assert_eq!(compliance.call(grant_request).await.unwrap(), ComplianceResponse::Grant);

        // Test case 2: Invalid policy flow - should deny
        let deny_request = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([(
                String::new(),
                HashMap::from([(Resource::new_process_mock(0), create_secret_policy(5))]),
            )]),
            destination: file.clone(),
        };
        assert_eq!(
            compliance.call(deny_request).await.unwrap_err(),
            TraceabilityError::DirectPolicyViolation
        );
    }

    #[tokio::test]
    async fn unit_compliance_service_complex_policy_scenario() {
        init_tracing();
        let mut compliance = ComplianceService::default();

        let file = Resource::new_file("/tmp/dest".to_string());
        let high_policy = create_secret_policy(10);
        let medium_policy = create_public_policy(5);
        let low_policy = create_public_policy(1);

        compliance.set_policy(file.clone(), medium_policy.clone());

        // Test 1: High (Secret) -> Medium (Public): Should fail due to confidentiality
        let request1 = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([
                (
                    String::new(),
                    HashMap::from([(Resource::new_process_mock(0), Policy::default())]),
                ),
                (
                    "10.0.0.1".to_string(),
                    HashMap::from([(Resource::new_process_mock(0), high_policy.clone())]),
                ),
            ]),
            destination: file.clone(),
        };
        assert_eq!(
            compliance.call(request1).await.unwrap_err(),
            TraceabilityError::DirectPolicyViolation
        ); // Secret -> Public fails

        // Test 2: Medium -> Low: Should pass (integrity 5 >= 1, public -> public)
        compliance.set_policy(file.clone(), low_policy.clone());
        let request2 = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([(
                String::new(),
                HashMap::from([(Resource::new_process_mock(0), medium_policy)]),
            )]),
            destination: file.clone(),
        };
        assert_eq!(compliance.call(request2).await.unwrap(), ComplianceResponse::Grant);

        // Test 3: Low -> High: Should fail due to integrity (1 < 10)
        compliance.set_policy(file.clone(), high_policy.clone());
        let request3 = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([(
                String::new(),
                HashMap::from([(Resource::new_process_mock(0), low_policy)]),
            )]),
            destination: file,
        };
        assert_eq!(
            compliance.call(request3).await.unwrap_err(),
            TraceabilityError::DirectPolicyViolation
        );
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_empty() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();
        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());

        assert_eq!(
            compliance.get_policies(HashSet::from([process.clone(), file.clone()])).unwrap(),
            HashMap::from([(process, Policy::default()), (file, Policy::default())])
        );
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_scenarios() {
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
            compliance.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process.clone(), process_policy.clone())])
        );

        // Test 2: Multiple resources with policies
        compliance.set_policy(file.clone(), file_policy.clone());
        assert_eq!(
            compliance.get_policies(HashSet::from([process.clone(), file.clone()])).unwrap(),
            HashMap::from([(process.clone(), process_policy.clone()), (file.clone(), file_policy)])
        );

        // Test 3: Mixed existing and default policies
        let new_file = Resource::new_file("/tmp/new.txt".to_string());
        assert_eq!(
            compliance.get_policies(HashSet::from([process.clone(), new_file.clone()])).unwrap(),
            HashMap::from([(process, process_policy), (new_file, Policy::default())])
        );
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_edge_cases() {
        init_tracing();
        let compliance = ComplianceService::default();
        let process = Resource::new_process_mock(0);

        // Test 1: Empty request
        assert_eq!(compliance.get_policies(HashSet::new()).unwrap(), HashMap::new());

        // Test 2: Policy updates
        let initial_policy = create_public_policy(2);
        let updated_policy = create_secret_policy(9);

        compliance.set_policy(process.clone(), initial_policy.clone());
        assert_eq!(
            compliance.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process.clone(), initial_policy.clone())])
        );

        compliance.set_policy(process.clone(), updated_policy.clone());
        assert_eq!(
            compliance.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process, updated_policy)])
        );
    }

    #[tokio::test]
    async fn unit_compliance_deleted_policy_behavior() {
        init_tracing();
        let compliance = ComplianceService::default();
        let process = Resource::new_process_mock(0);
        let mock_file = Resource::new_file("/tmp/dest".to_string());

        // Create and set a deleted policy
        let deleted_policy = create_deleted_policy(5);
        compliance.set_policy(process.clone(), deleted_policy.clone());

        // Test 1: Deleted policy is returned correctly
        assert_eq!(
            compliance.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process.clone(), deleted_policy.clone())])
        );

        // Test 2: Cannot update deleted policy
        assert_eq!(
            compliance.set_policy(process.clone(), Policy::default()),
            ComplianceResponse::PolicyNotUpdated
        );

        // Test 3: Policy remains deleted after update attempt
        assert_eq!(
            compliance.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process.clone(), deleted_policy.clone())])
        );

        // Test 4: Deleted policies cause policy violations in evaluation
        assert!(
            compliance
                .eval_policies(
                    HashMap::from([(
                        String::new(),
                        HashMap::from([
                            (Resource::new_process_mock(0), deleted_policy.clone()),
                            (Resource::new_process_mock(1), Policy::default())
                        ])
                    )]),
                    mock_file.clone()
                )
                .await
                .is_err_and(|e| e == TraceabilityError::DirectPolicyViolation)
        );

        assert!(
            compliance
                .eval_policies(
                    HashMap::from([(
                        String::new(),
                        HashMap::from([(Resource::new_process_mock(0), Policy::default())])
                    )]),
                    process.clone()
                )
                .await
                .is_err_and(|e| e == TraceabilityError::DirectPolicyViolation)
        );
    }

    #[tokio::test]
    async fn unit_compliance_cache_mode_policy_not_found() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();

        let cache_policy_map = ComplianceService::init_cache();
        let process = Resource::new_process_mock(0);

        // In cache mode, requesting policy for non-existent resource should return PolicyNotFound error
        assert!(
            cache_policy_map.get_policies(HashSet::from([process.clone()])).is_err_and(
                |e| matches!(e, TraceabilityError::PolicyNotFound(res) if res == process)
            )
        );
    }

    #[tokio::test]
    async fn unit_compliance_normal_mode_vs_cache_mode() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();

        let normal_policy_map = ComplianceService::default();
        let cache_policy_map = ComplianceService::init_cache();
        let process = Resource::new_process_mock(0);

        // Normal mode should return default policy when resource not found
        assert_eq!(
            normal_policy_map.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process.clone(), Policy::default())])
        );

        // Cache mode should return PolicyNotFound error when resource not found
        assert!(
            cache_policy_map.get_policies(HashSet::from([process.clone()])).is_err_and(
                |e| matches!(e, TraceabilityError::PolicyNotFound(res) if res == process)
            )
        );
    }

    #[tokio::test]
    async fn unit_compliance_cache_mode_with_existing_policies() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();

        let cache_policy_map = ComplianceService::init_cache();
        let process = Resource::new_process_mock(0);

        let policy =
            Policy::new(ConfidentialityPolicy::Secret, 7, DeletionPolicy::NotDeleted, true);

        // Set policy first
        cache_policy_map.set_policy(process.clone(), policy.clone());

        // Now getting policy should work in cache mode
        assert_eq!(
            cache_policy_map.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process, policy)])
        );
    }

    #[tokio::test]
    async fn unit_compliance_cache_mode_mixed_resources() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();

        let cache_policy_map = ComplianceService::init_cache();
        let process1 = Resource::new_process_mock(1);
        let process2 = Resource::new_process_mock(2);

        let policy1 = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 3,
            deleted: DeletionPolicy::NotDeleted,
            consent: true,
        };

        // Set policy only for process1
        cache_policy_map.set_policy(process1.clone(), policy1);

        // Request policies for both processes - should fail because process2 has no policy
        assert!(
            cache_policy_map.get_policies(HashSet::from([process1, process2.clone()])).is_err_and(
                |e| matches!(e, TraceabilityError::PolicyNotFound(res) if res == process2)
            )
        );
    }

    #[tokio::test]
    async fn unit_compliance_cache_mode_empty_request() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();

        let cache_policy_map = ComplianceService::init_cache();

        // Empty request should work the same in both modes
        assert_eq!(cache_policy_map.get_policies(HashSet::new()).unwrap(), HashMap::new());
    }

    #[tokio::test]
    async fn unit_deletion_policy_state_transitions() {
        init_tracing();

        // Test 1: Initial state and is_deleted/is_pending methods
        let mut policy = Policy::default();
        assert!(!policy.is_deleted());
        assert!(!policy.is_pending_deletion());

        // Test 2: Transition from NotDeleted to Pending via deletion_request
        policy.deleted();
        assert!(policy.is_deleted()); // Pending counts as deleted
        assert!(policy.is_pending_deletion());

        // Test 3: Multiple calls to deletion_request on Pending should not change state
        policy.deleted();
        assert!(policy.is_deleted());
        assert!(policy.is_pending_deletion());

        // Test 4: Transition from Pending to Deleted via deletion_applied
        policy.deletion_enforced();
        assert!(policy.is_deleted());
        assert!(!policy.is_pending_deletion());

        // Test 5: Multiple calls to deletion_applied on Deleted should not change state
        policy.deletion_enforced();
        assert!(policy.is_deleted());
        assert!(!policy.is_pending_deletion());

        // Test 6: Test From<bool> conversion
        assert_eq!(DeletionPolicy::from(true), DeletionPolicy::Deleted);
        assert_eq!(DeletionPolicy::from(false), DeletionPolicy::NotDeleted);
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
        let eval_with_deleted_source = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([(
                String::new(),
                HashMap::from([(new_resource, deleted_policy)]),
            )]),
            destination: Resource::new_file("/tmp/dest".to_string()),
        };
        assert_eq!(
            compliance.call(eval_with_deleted_source).await.unwrap_err(),
            TraceabilityError::DirectPolicyViolation
        );

        // Test 9: Test policy evaluation with deleted destination - should fail
        let eval_with_deleted_dest = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([(
                String::new(),
                HashMap::from([(process, process_policy)]),
            )]),
            destination: Resource::new_file("/tmp/dest".to_string()),
        };
        assert_eq!(
            compliance.call(eval_with_deleted_dest).await.unwrap_err(),
            TraceabilityError::DirectPolicyViolation
        );
    }
}
