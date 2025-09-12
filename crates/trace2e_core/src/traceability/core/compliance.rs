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
#[derive(Default, PartialEq, Debug, Clone, Eq, Hash)]
pub enum ConfidentialityPolicy {
    Secret,
    #[default]
    Public,
}

/// Policy for a resource
///
/// This policy is used to check the compliance of input/output flows of the associated resource.
#[derive(Default, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Policy {
    pub confidentiality: ConfidentialityPolicy,
    pub integrity: u32,
    pub deleted: bool,
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
    fn set_policy(
        &self,
        resource: Resource,
        policy: Policy,
    ) -> Result<ComplianceResponse, TraceabilityError> {
        // Enforce deleted policy
        if self.policies.get(&resource).is_some_and(|p| p.deleted) {
            return Ok(ComplianceResponse::PolicyNotUpdated);
        }
        self.policies.insert(resource, policy);
        Ok(ComplianceResponse::PolicyUpdated)
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
            if source_policy.deleted || destination_policy.deleted {
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
                    this.policies.set_policy(resource, policy)
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
        Policy { confidentiality: ConfidentialityPolicy::Public, integrity, deleted: false }
    }

    fn create_secret_policy(integrity: u32) -> Policy {
        Policy { confidentiality: ConfidentialityPolicy::Secret, integrity, deleted: false }
    }

    fn create_deleted_policy(integrity: u32) -> Policy {
        Policy { confidentiality: ConfidentialityPolicy::Public, integrity, deleted: true }
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
            compliance.policies.set_policy(process.clone(), policy).unwrap(),
            ComplianceResponse::PolicyUpdated
        );

        let new_policy = create_secret_policy(3);

        // Updating existing policy should also return PolicyUpdated
        assert_eq!(
            compliance.policies.set_policy(process, new_policy).unwrap(),
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
        compliance.policies.set_policy(process.clone(), process_policy.clone()).unwrap();
        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process.clone(), process_policy.clone())])
        );

        // Test 2: Multiple resources with policies
        compliance.policies.set_policy(file.clone(), file_policy.clone()).unwrap();
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

        compliance.policies.set_policy(process.clone(), initial_policy.clone()).unwrap();
        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process.clone(), initial_policy.clone())])
        );

        compliance.policies.set_policy(process.clone(), updated_policy.clone()).unwrap();
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
        compliance.policies.set_policy(process.clone(), deleted_policy.clone()).unwrap();

        // Test 1: Deleted policy is returned correctly
        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process.clone()])).unwrap(),
            HashMap::from([(process.clone(), deleted_policy.clone())])
        );

        // Test 2: Cannot update deleted policy
        assert_eq!(
            compliance.policies.set_policy(process.clone(), Policy::default()).unwrap(),
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
            Policy { confidentiality: ConfidentialityPolicy::Secret, integrity: 7, deleted: false };

        // Set policy first
        cache_policy_map.set_policy(process.clone(), policy.clone()).unwrap();

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

        let policy1 =
            Policy { confidentiality: ConfidentialityPolicy::Public, integrity: 3, deleted: false };

        // Set policy only for process1
        cache_policy_map.set_policy(process1.clone(), policy1).unwrap();

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
}
