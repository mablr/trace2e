use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use dashmap::DashMap;
use tower::Service;
#[cfg(feature = "trace2e_tracing")]
use tracing::info;

use crate::traceability::{
    api::{ComplianceRequest, ComplianceResponse},
    error::TraceabilityError,
    naming::Resource,
};

/// Confidentiality policy defines the level of confidentiality of a resource
#[derive(Default, PartialEq, Debug, Clone, Copy, Eq)]
pub enum ConfidentialityPolicy {
    Secret,
    #[default]
    Public,
}

/// Deletion policy defines the deletion status of a resource
#[derive(Default, PartialEq, Debug, Clone, Eq, Copy)]
pub enum DeletionPolicy {
    #[default]
    NotDeleted,
    Pending,
    Deleted,
}

impl From<bool> for DeletionPolicy {
    fn from(deleted: bool) -> Self {
        if deleted { DeletionPolicy::Deleted } else { DeletionPolicy::NotDeleted }
    }
}

/// Policy for a resource
///
/// This policy is used to check the compliance of input/output flows of the associated resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Policy {
    confidentiality: ConfidentialityPolicy,
    integrity: u32,
    deleted: DeletionPolicy,
    // Whether the owner has given consent for flows involving this resource.
    // Default: true (consent given). When false, flows should be denied.
    consent: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 0,
            deleted: DeletionPolicy::NotDeleted,
            consent: true,
        }
    }
}

impl Policy {
    pub fn new(
        confidentiality: ConfidentialityPolicy,
        integrity: u32,
        deleted: DeletionPolicy,
        consent: bool,
    ) -> Self {
        Self { confidentiality, integrity, deleted, consent }
    }

    /// Returns the confidentiality policy
    /// Returns true if the resource is confidential
    pub fn is_confidential(&self) -> bool {
        self.confidentiality == ConfidentialityPolicy::Secret
    }

    /// Returns true if the resource is deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted != DeletionPolicy::NotDeleted
    }

    /// Returns true if the resource is pending deletion
    pub fn is_pending_deletion(&self) -> bool {
        self.deleted == DeletionPolicy::Pending
    }

    /// Returns true if the resource is pending deletion
    pub fn get_integrity(&self) -> u32 {
        self.integrity
    }

    /// Returns true if owner has given consent for flows involving this resource
    pub fn get_consent(&self) -> bool {
        self.consent
    }

    pub fn with_consent(&mut self, consent: bool) -> ComplianceResponse {
        if !self.is_deleted() {
            self.consent = consent;
            ComplianceResponse::PolicyUpdated
        } else {
            ComplianceResponse::PolicyNotUpdated
        }
    }
    pub fn with_integrity(&mut self, integrity: u32) -> ComplianceResponse {
        if !self.is_deleted() {
            self.integrity = integrity;
            ComplianceResponse::PolicyUpdated
        } else {
            ComplianceResponse::PolicyNotUpdated
        }
    }

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

    /// Request the deletion of the resource
    pub fn deleted(&mut self) -> ComplianceResponse {
        if !self.is_deleted() {
            self.deleted = DeletionPolicy::Pending;
            ComplianceResponse::PolicyUpdated
        } else {
            ComplianceResponse::PolicyNotUpdated
        }
    }

    pub fn deletion_enforced(&mut self) -> ComplianceResponse {
        if self.is_pending_deletion() {
            self.deleted = DeletionPolicy::Deleted;
            ComplianceResponse::PolicyUpdated
        } else {
            ComplianceResponse::PolicyNotUpdated
        }
    }
}

#[derive(Default, Clone, Debug)]
struct PolicyMap {
    cache_mode: bool,
    policies: Arc<DashMap<Resource, Policy>>,
}

impl PolicyMap {
    /// Create a new PolicyMap in cache mode for testing purposes
    #[cfg(test)]
    fn init_cache() -> Self {
        Self { cache_mode: true, policies: Arc::new(DashMap::new()) }
    }

    /// Create a new PolicyMap in cache mode
    fn cache(policies: Arc<DashMap<Resource, Policy>>) -> Self {
        Self { cache_mode: true, policies }
    }

    /// Get the policy for a specific resource
    /// It returns a PolicyNotFound error if the resource is not found in cache mode
    /// It returns the default policy if the resource is not found in normal mode
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

    /// Get the policies for a set of resources
    /// It returns a PolicyNotFound error if the resource is not found in cache mode
    /// It returns the default policy if the resource is not found in normal mode
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

    /// Set the policy for a specific resource
    /// Returns the old policy if the resource is already set
    fn set_policy(&self, resource: Resource, policy: Policy) -> ComplianceResponse {
        // Enforce deleted policy
        if self.policies.get(&resource).is_some_and(|policy| policy.is_deleted()) {
            return ComplianceResponse::PolicyNotUpdated;
        }
        self.policies.insert(resource, policy);
        ComplianceResponse::PolicyUpdated
    }

    /// Set the confidentiality for a specific resource
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

    /// Set the integrity for a specific resource
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

    /// Set a specific resource as deleted
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

    /// Set consent flag for a specific resource
    fn set_consent(&self, resource: Resource, consent: bool) -> ComplianceResponse {
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

impl From<HashMap<Resource, Policy>> for PolicyMap {
    fn from(map: HashMap<Resource, Policy>) -> Self {
        let dash_map = DashMap::from_iter(map);
        Self::cache(Arc::new(dash_map))
    }
}

/// Helper function to evaluate the compliance of policies for a flow from source to destination
/// Returns true if the flow is compliant, false otherwise
fn eval_policies(
    source_policies: HashMap<String, HashMap<Resource, Policy>>,
    destination_policy: Policy,
) -> Result<ComplianceResponse, TraceabilityError> {
    // Merge local and remote source policies, ignoring the node
    // TODO: implement node based policies
    for source_policy_batch in source_policies.values() {
        for source_policy in source_policy_batch.values() {
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

    Ok(ComplianceResponse::Grant)
}

/// Compliance service for managing and checking policies
#[derive(Default, Clone, Debug)]
pub struct ComplianceService {
    policies: PolicyMap,
}

impl Service<ComplianceRequest> for ComplianceService {
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
                ComplianceRequest::EvalPolicies { source_policies, destination_policy } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance] CheckCompliance: source_policies: {:?}, destination_policy: {:?}",
                        source_policies, destination_policy
                    );
                    eval_policies(source_policies, destination_policy)
                }
                ComplianceRequest::GetPolicy(resource) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance] GetPolicy: resource: {:?}", resource);
                    Ok(ComplianceResponse::Policy(this.policies.get_policy(&resource)?))
                }
                ComplianceRequest::GetPolicies(resources) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance] GetPolicies: resources: {:?}", resources);
                    Ok(ComplianceResponse::Policies(this.policies.get_policies(resources)?))
                }
                ComplianceRequest::SetPolicy { resource, policy } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance] SetPolicy: resource: {:?}, policy: {:?}", resource, policy);
                    Ok(this.policies.set_policy(resource, policy))
                }
                ComplianceRequest::SetConfidentiality { resource, confidentiality } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance] SetConfidentiality: resource: {:?}, confidentiality: {:?}",
                        resource, confidentiality
                    );
                    Ok(this.policies.set_confidentiality(resource, confidentiality))
                }
                ComplianceRequest::SetIntegrity { resource, integrity } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance] SetIntegrity: resource: {:?}, integrity: {:?}",
                        resource, integrity
                    );
                    Ok(this.policies.set_integrity(resource, integrity))
                }
                ComplianceRequest::SetDeleted(resource) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance] SetDeleted: resource: {:?}", resource);
                    Ok(this.policies.set_deleted(resource))
                }
                ComplianceRequest::SetConsent { resource, consent } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance] SetConsent: resource: {:?}, consent: {:?}",
                        resource, consent
                    );
                    Ok(this.policies.set_consent(resource, consent))
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traceability::naming::Resource;

    // Helper functions to reduce test code duplication
    fn create_public_policy(integrity: u32) -> Policy {
        Policy::new(ConfidentialityPolicy::Public, integrity, DeletionPolicy::NotDeleted, true)
    }

    fn create_secret_policy(integrity: u32) -> Policy {
        Policy::new(ConfidentialityPolicy::Secret, integrity, DeletionPolicy::NotDeleted, true)
    }

    fn create_deleted_policy(integrity: u32) -> Policy {
        Policy::new(ConfidentialityPolicy::Public, integrity, DeletionPolicy::Pending, true)
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
            compliance.policies.set_policy(process.clone(), policy),
            ComplianceResponse::PolicyUpdated
        );

        let new_policy = create_secret_policy(3);

        // Updating existing policy should also return PolicyUpdated
        assert_eq!(
            compliance.policies.set_policy(process, new_policy),
            ComplianceResponse::PolicyUpdated
        );
    }

    #[tokio::test]
    async fn unit_compliance_policy_evaluation_scenarios() {
        init_tracing();

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

        for (source_policy, dest_policy, should_pass, description) in test_cases {
            let result = eval_policies(
                HashMap::from([(
                    String::new(),
                    HashMap::from([(Resource::new_process_mock(0), source_policy)]),
                )]),
                dest_policy,
            );

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

        // Test 1: All default policies should pass
        assert!(
            eval_policies(
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
                Policy::default()
            )
            .is_ok_and(|r| r == ComplianceResponse::Grant)
        );

        // Test 2: Mixed default and explicit policies - integrity violation
        let dest_policy = create_public_policy(2);
        assert!(
            eval_policies(
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
                dest_policy // integrity: 2, so 0 < 2 should fail
            )
            .is_err_and(|e| e == TraceabilityError::DirectPolicyViolation)
        );
    }

    #[tokio::test]
    async fn unit_compliance_service_eval_policies_requests() {
        init_tracing();
        let mut compliance = ComplianceService::default();

        // Test case 1: Valid policy flow - should grant
        let grant_request = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([(
                String::new(),
                HashMap::from([(Resource::new_process_mock(0), create_public_policy(5))]),
            )]),
            destination_policy: create_public_policy(3),
        };
        assert_eq!(compliance.call(grant_request).await.unwrap(), ComplianceResponse::Grant);

        // Test case 2: Invalid policy flow - should deny
        let deny_request = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([(
                String::new(),
                HashMap::from([(Resource::new_process_mock(0), create_secret_policy(5))]),
            )]),
            destination_policy: create_public_policy(3),
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

        let high_policy = create_secret_policy(10);
        let medium_policy = create_public_policy(5);
        let low_policy = create_public_policy(1);

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
            destination_policy: medium_policy.clone(),
        };
        assert_eq!(
            compliance.call(request1).await.unwrap_err(),
            TraceabilityError::DirectPolicyViolation
        ); // Secret -> Public fails

        // Test 2: Medium -> Low: Should pass (integrity 5 >= 1, public -> public)
        let request2 = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([(
                String::new(),
                HashMap::from([(Resource::new_process_mock(0), medium_policy)]),
            )]),
            destination_policy: low_policy.clone(),
        };
        assert_eq!(compliance.call(request2).await.unwrap(), ComplianceResponse::Grant);

        // Test 3: Low -> High: Should fail due to integrity (1 < 10)
        let request3 = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([(
                String::new(),
                HashMap::from([(Resource::new_process_mock(0), low_policy)]),
            )]),
            destination_policy: high_policy,
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
            compliance
                .policies
                .get_policies(HashSet::from([process.clone(), file.clone()]))
                .unwrap(),
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
        compliance.policies.set_policy(process.clone(), process_policy.clone());
        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process.clone(), process_policy.clone())])
        );

        // Test 2: Multiple resources with policies
        compliance.policies.set_policy(file.clone(), file_policy.clone());
        assert_eq!(
            compliance
                .policies
                .get_policies(HashSet::from([process.clone(), file.clone()]))
                .unwrap(),
            HashMap::from([(process.clone(), process_policy.clone()), (file.clone(), file_policy)])
        );

        // Test 3: Mixed existing and default policies
        let new_file = Resource::new_file("/tmp/new.txt".to_string());
        assert_eq!(
            compliance
                .policies
                .get_policies(HashSet::from([process.clone(), new_file.clone()]))
                .unwrap(),
            HashMap::from([(process, process_policy), (new_file, Policy::default())])
        );
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_edge_cases() {
        init_tracing();
        let compliance = ComplianceService::default();
        let process = Resource::new_process_mock(0);

        // Test 1: Empty request
        assert_eq!(compliance.policies.get_policies(HashSet::new()).unwrap(), HashMap::new());

        // Test 2: Policy updates
        let initial_policy = create_public_policy(2);
        let updated_policy = create_secret_policy(9);

        compliance.policies.set_policy(process.clone(), initial_policy.clone());
        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process.clone(), initial_policy.clone())])
        );

        compliance.policies.set_policy(process.clone(), updated_policy.clone());
        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process, updated_policy)])
        );
    }

    #[tokio::test]
    async fn unit_compliance_deleted_policy_behavior() {
        init_tracing();
        let compliance = ComplianceService::default();
        let process = Resource::new_process_mock(0);

        // Create and set a deleted policy
        let deleted_policy = create_deleted_policy(5);
        compliance.policies.set_policy(process.clone(), deleted_policy.clone());

        // Test 1: Deleted policy is returned correctly
        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process.clone(), deleted_policy.clone())])
        );

        // Test 2: Cannot update deleted policy
        assert_eq!(
            compliance.policies.set_policy(process.clone(), Policy::default()),
            ComplianceResponse::PolicyNotUpdated
        );

        // Test 3: Policy remains deleted after update attempt
        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process, deleted_policy.clone())])
        );

        // Test 4: Deleted policies cause policy violations in evaluation
        assert!(
            eval_policies(
                HashMap::from([(
                    String::new(),
                    HashMap::from([
                        (Resource::new_process_mock(0), deleted_policy.clone()),
                        (Resource::new_process_mock(1), Policy::default())
                    ])
                )]),
                Policy::default()
            )
            .is_err_and(|e| e == TraceabilityError::DirectPolicyViolation)
        );

        assert!(
            eval_policies(
                HashMap::from([(
                    String::new(),
                    HashMap::from([(Resource::new_process_mock(0), Policy::default())])
                )]),
                deleted_policy
            )
            .is_err_and(|e| e == TraceabilityError::DirectPolicyViolation)
        );
    }

    #[tokio::test]
    async fn unit_compliance_cache_mode_policy_not_found() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();

        let cache_policy_map = PolicyMap::init_cache();
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

        let normal_policy_map = PolicyMap::default();
        let cache_policy_map = PolicyMap::init_cache();
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

        let cache_policy_map = PolicyMap::init_cache();
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

        let cache_policy_map = PolicyMap::init_cache();
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

        let cache_policy_map = PolicyMap::init_cache();

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
            destination_policy: create_public_policy(1),
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
            destination_policy: pending_deletion_policy,
        };
        assert_eq!(
            compliance.call(eval_with_deleted_dest).await.unwrap_err(),
            TraceabilityError::DirectPolicyViolation
        );
    }
}
