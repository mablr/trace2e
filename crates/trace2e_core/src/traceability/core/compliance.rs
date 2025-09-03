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
pub struct PolicyMap {
    policies: Arc<DashMap<Resource, Policy>>,
}

impl PolicyMap {
    /// Get the policies for a specific resource
    /// Returns the default policy if the resource is not found
    async fn get_policies(
        &self,
        resources: HashSet<Resource>,
    ) -> Result<ComplianceResponse, TraceabilityError> {
        let mut policies_set = HashSet::new();
        for resource in resources {
            // Get the policy from the local policies, streams have no policies
            if resource.is_stream().is_none() {
                let policy = self.policies.get(&resource).map(|p| p.clone()).unwrap_or_default();
                policies_set.insert(policy);
            }
        }
        Ok(ComplianceResponse::Policies(policies_set))
    }

    /// Set the policy for a specific resource
    /// Returns the old policy if the resource is already set
    async fn set_policy(
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

/// Compliance service for managing policies
///
/// This service is responsible for managing the policies for the resources.
/// It is also used to check the compliance of a flow.
#[derive(Default, Clone, Debug)]
pub struct ComplianceService {
    policies: PolicyMap,
}

impl ComplianceService {
    /// Evaluate the compliance of policies for a flow from source to destination
    /// Returns true if the flow is compliant, false otherwise
    async fn eval_policies(
        &self,
        source_policies: HashMap<String, HashSet<Policy>>,
        destination_policy: Policy,
    ) -> Result<ComplianceResponse, TraceabilityError> {
        // Merge local and remote source policies, ignoring the node
        // TODO: implement node based policies
        for source_policy in source_policies.values().flatten() {
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

        Ok(ComplianceResponse::Grant)
    }
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
                    this.eval_policies(source_policies, destination_policy).await
                }
                ComplianceRequest::GetPolicies(resources) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance] GetPolicies: resources: {:?}", resources);
                    this.policies.get_policies(resources).await
                }
                ComplianceRequest::SetPolicy { resource, policy } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance] SetPolicy: resource: {:?}, policy: {:?}", resource, policy);
                    this.policies.set_policy(resource, policy).await
                }
                ComplianceRequest::CheckCompliance { .. } => Err(TraceabilityError::InvalidRequest),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traceability::naming::Resource;

    #[tokio::test]
    async fn unit_compliance_set_policy_basic() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();
        let process = Resource::new_process_mock(0);

        let policy =
            Policy { confidentiality: ConfidentialityPolicy::Secret, integrity: 5, deleted: false };

        // First time setting policy should return None
        assert_eq!(
            compliance.policies.set_policy(process.clone(), policy.clone()).await.unwrap(),
            ComplianceResponse::PolicyUpdated
        );

        let new_policy =
            Policy { confidentiality: ConfidentialityPolicy::Secret, integrity: 3, deleted: false };

        // Second time setting policy should return the old policy
        assert_eq!(
            compliance.policies.set_policy(process, new_policy).await.unwrap(),
            ComplianceResponse::PolicyUpdated
        );
    }

    #[tokio::test]
    async fn unit_compliance_check_integrity_pass() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();

        let source_policy =
            Policy { confidentiality: ConfidentialityPolicy::Public, integrity: 5, deleted: false };

        let dest_policy =
            Policy { confidentiality: ConfidentialityPolicy::Public, integrity: 3, deleted: false };

        assert!(
            compliance
                .eval_policies(
                    HashMap::from([(String::new(), HashSet::from([source_policy]))]),
                    dest_policy
                )
                .await
                .is_ok_and(|r| r == ComplianceResponse::Grant)
        );
    }

    #[tokio::test]
    async fn unit_compliance_check_integrity_fail() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();

        let source_policy =
            Policy { confidentiality: ConfidentialityPolicy::Public, integrity: 3, deleted: false };

        let dest_policy =
            Policy { confidentiality: ConfidentialityPolicy::Public, integrity: 5, deleted: false };

        assert!(
            compliance
                .eval_policies(
                    HashMap::from([(String::new(), HashSet::from([source_policy]))]),
                    dest_policy
                )
                .await
                .is_err_and(|e| e == TraceabilityError::DirectPolicyViolation)
        );
    }

    #[tokio::test]
    async fn unit_compliance_check_confidentiality_pass() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();

        let source_policy =
            Policy { confidentiality: ConfidentialityPolicy::Secret, integrity: 5, deleted: false };

        let dest_policy =
            Policy { confidentiality: ConfidentialityPolicy::Secret, integrity: 3, deleted: false };

        assert!(
            compliance
                .eval_policies(
                    HashMap::from([(String::new(), HashSet::from([source_policy]))]),
                    dest_policy
                )
                .await
                .is_ok_and(|r| r == ComplianceResponse::Grant)
        );
    }

    #[tokio::test]
    async fn unit_compliance_check_confidentiality_fail() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();

        let source_policy =
            Policy { confidentiality: ConfidentialityPolicy::Secret, integrity: 5, deleted: false };

        let dest_policy =
            Policy { confidentiality: ConfidentialityPolicy::Public, integrity: 3, deleted: false };

        assert!(
            compliance
                .eval_policies(
                    HashMap::from([(String::new(), HashSet::from([source_policy]))]),
                    dest_policy
                )
                .await
                .is_err_and(|e| e == TraceabilityError::DirectPolicyViolation)
        );
    }

    #[tokio::test]
    async fn unit_compliance_check_default_policies() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();

        // Both should use default policies (Public, integrity 0)
        assert!(
            compliance
                .eval_policies(
                    HashMap::from([
                        (String::new(), HashSet::from([Policy::default(), Policy::default()])),
                        ("10.0.0.1".to_string(), HashSet::from([Policy::default()]))
                    ]),
                    Policy::default()
                )
                .await
                .is_ok_and(|r| r == ComplianceResponse::Grant)
        );
    }

    #[tokio::test]
    async fn unit_compliance_check_mixed_default_explicit() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();

        let dest_policy =
            Policy { confidentiality: ConfidentialityPolicy::Public, integrity: 2, deleted: false };

        // Source uses default (integrity 0), destination has integrity 2
        assert!(
            compliance
                .eval_policies(
                    HashMap::from([
                        (String::new(), HashSet::from([Policy::default()])),
                        ("10.0.0.1".to_string(), HashSet::from([Policy::default()]))
                    ]),
                    dest_policy
                )
                .await
                .is_err_and(|e| e == TraceabilityError::DirectPolicyViolation)
        );
    }

    #[tokio::test]
    async fn unit_compliance_service_request_grant() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut compliance = ComplianceService::default();

        let source_policy =
            Policy { confidentiality: ConfidentialityPolicy::Public, integrity: 5, deleted: false };

        let dest_policy =
            Policy { confidentiality: ConfidentialityPolicy::Public, integrity: 3, deleted: false };

        let request = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([(String::new(), HashSet::from([source_policy]))]),
            destination_policy: dest_policy,
        };

        assert_eq!(compliance.call(request).await.unwrap(), ComplianceResponse::Grant);
    }

    #[tokio::test]
    async fn unit_compliance_service_request_deny() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut compliance = ComplianceService::default();

        let source_policy =
            Policy { confidentiality: ConfidentialityPolicy::Secret, integrity: 5, deleted: false };

        let dest_policy =
            Policy { confidentiality: ConfidentialityPolicy::Public, integrity: 3, deleted: false };

        let request = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([(String::new(), HashSet::from([source_policy]))]),
            destination_policy: dest_policy,
        };

        assert_eq!(
            compliance.call(request).await.unwrap_err(),
            TraceabilityError::DirectPolicyViolation
        );
    }

    #[tokio::test]
    async fn unit_compliance_service_complex_policy_scenario() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut compliance = ComplianceService::default();

        let high_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 10,
            deleted: false,
        };

        let medium_policy =
            Policy { confidentiality: ConfidentialityPolicy::Public, integrity: 5, deleted: false };

        let low_policy =
            Policy { confidentiality: ConfidentialityPolicy::Public, integrity: 1, deleted: false };

        // High -> Medium: Should pass (integrity 10 >= 5, secret -> public is blocked but this is
        // reverse)
        let request1 = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([
                (String::new(), HashSet::from([Policy::default()])),
                ("10.0.0.1".to_string(), HashSet::from([high_policy.clone()])),
            ]),
            destination_policy: medium_policy.clone(),
        };
        assert_eq!(
            compliance.call(request1).await.unwrap_err(),
            TraceabilityError::DirectPolicyViolation
        ); // Secret -> Public fails

        // Medium -> Low: Should pass (integrity 5 >= 1, public -> public)
        let request2 = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([(String::new(), HashSet::from([medium_policy]))]),
            destination_policy: low_policy.clone(),
        };
        assert_eq!(compliance.call(request2).await.unwrap(), ComplianceResponse::Grant);

        // Low -> High: Should fail (integrity 1 < 10)
        let request3 = ComplianceRequest::EvalPolicies {
            source_policies: HashMap::from([(String::new(), HashSet::from([low_policy]))]),
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
            compliance.policies.get_policies(HashSet::from([process, file])).await.unwrap(),
            ComplianceResponse::Policies(HashSet::from([Policy::default()]))
        );
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_single_resource() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();
        let process = Resource::new_process_mock(0);

        let policy =
            Policy { confidentiality: ConfidentialityPolicy::Secret, integrity: 7, deleted: false };

        compliance.policies.set_policy(process.clone(), policy.clone()).await.unwrap();

        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process])).await.unwrap(),
            ComplianceResponse::Policies(HashSet::from([policy]))
        );
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_multiple_resources() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();
        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());

        let process_policy =
            Policy { confidentiality: ConfidentialityPolicy::Secret, integrity: 5, deleted: false };

        let file_policy =
            Policy { confidentiality: ConfidentialityPolicy::Public, integrity: 3, deleted: false };

        compliance.policies.set_policy(process.clone(), process_policy.clone()).await.unwrap();
        compliance.policies.set_policy(file.clone(), file_policy.clone()).await.unwrap();

        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process, file])).await.unwrap(),
            ComplianceResponse::Policies(HashSet::from([process_policy, file_policy]))
        );
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_mixed_existing_default() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();
        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());

        let process_policy =
            Policy { confidentiality: ConfidentialityPolicy::Secret, integrity: 8, deleted: false };

        // Set policy only for process, not for file
        compliance.policies.set_policy(process.clone(), process_policy.clone()).await.unwrap();

        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process, file])).await.unwrap(),
            ComplianceResponse::Policies(HashSet::from([process_policy, Policy::default()]))
        );
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_empty_request() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();
        assert_eq!(
            compliance.policies.get_policies(HashSet::new()).await.unwrap(),
            ComplianceResponse::Policies(HashSet::new())
        );
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_after_update() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();
        let process = Resource::new_process_mock(0);

        let initial_policy =
            Policy { confidentiality: ConfidentialityPolicy::Public, integrity: 2, deleted: false };

        let updated_policy =
            Policy { confidentiality: ConfidentialityPolicy::Secret, integrity: 9, deleted: false };

        // Set initial policy
        compliance.policies.set_policy(process.clone(), initial_policy.clone()).await.unwrap();

        // Verify initial policy
        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process.clone()])).await.unwrap(),
            ComplianceResponse::Policies(HashSet::from([initial_policy]))
        );

        // Update policy
        compliance.policies.set_policy(process.clone(), updated_policy.clone()).await.unwrap();

        // Verify updated policy
        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process])).await.unwrap(),
            ComplianceResponse::Policies(HashSet::from([updated_policy]))
        );
    }

    #[tokio::test]
    async fn unit_compliance_policy_deleted_properties() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();
        let process = Resource::new_process_mock(0);

        let policy =
            Policy { confidentiality: ConfidentialityPolicy::Secret, integrity: 5, deleted: true };

        compliance.policies.set_policy(process.clone(), policy.clone()).await.unwrap();

        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process.clone()])).await.unwrap(),
            ComplianceResponse::Policies(HashSet::from([policy.clone()]))
        );

        // Setting a deleted policy must not have any effect, so it returns None
        assert_eq!(
            compliance.policies.set_policy(process.clone(), Policy::default()).await.unwrap(),
            ComplianceResponse::PolicyNotUpdated
        );

        // Getting the policies must return the deleted policy
        assert_eq!(
            compliance.policies.get_policies(HashSet::from([process])).await.unwrap(),
            ComplianceResponse::Policies(HashSet::from([policy]))
        );
    }

    #[tokio::test]
    async fn unit_compliance_policy_deleted_check() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();

        let policy =
            Policy { confidentiality: ConfidentialityPolicy::Secret, integrity: 5, deleted: true };

        assert!(
            compliance
                .eval_policies(
                    HashMap::from([(
                        String::new(),
                        HashSet::from([policy.clone(), Policy::default()])
                    )]),
                    Policy::default()
                )
                .await
                .is_err_and(|e| e == TraceabilityError::DirectPolicyViolation)
        );

        assert!(
            compliance
                .eval_policies(
                    HashMap::from([(String::new(), HashSet::from([Policy::default()]))]),
                    policy
                )
                .await
                .is_err_and(|e| e == TraceabilityError::DirectPolicyViolation)
        );
    }
}
