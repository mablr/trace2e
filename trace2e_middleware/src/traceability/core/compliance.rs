use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use tokio::sync::Mutex;
use tower::Service;
#[cfg(feature = "trace2e_tracing")]
use tracing::info;

use crate::traceability::{
    api::{ComplianceRequest, ComplianceResponse},
    error::TraceabilityError,
    naming::Resource,
};

#[derive(Default, PartialEq, Debug, Clone, Eq, Hash)]
pub enum ConfidentialityPolicy {
    Secret,
    #[default]
    Public,
}

#[derive(Default, Debug, Clone, Eq, PartialEq, Hash)]
pub struct Policy {
    pub confidentiality: ConfidentialityPolicy,
    pub integrity: u32,
}

#[derive(Default, Clone, Debug)]
pub struct ComplianceService {
    policies: Arc<Mutex<HashMap<Resource, Policy>>>,
}

impl ComplianceService {
    /// Get the policies for a specific resource
    /// Returns the default policy if the resource is not found
    async fn get_policies(&self, resources: HashSet<Resource>) -> HashSet<Policy> {
        let policies = self.policies.lock().await;
        let mut policies_set = HashSet::new();
        for resource in resources {
            // Get the policy from the local policies, streams have no policies
            if resource.is_stream().is_none() {
                policies_set.insert(policies.get(&resource).cloned().unwrap_or_default());
            }
        }
        policies_set
    }

    /// Set the policy for a specific resource
    /// Returns the old policy if the resource is already set
    async fn set_policy(&self, resource: Resource, policy: Policy) -> Option<Policy> {
        let mut policies = self.policies.lock().await;
        policies.insert(resource, policy)
    }

    /// Check if a flow from source to destination is compliant with policies
    /// Returns true if the flow is compliant, false otherwise
    async fn compliance_check(
        &self,
        source_policies: HashMap<String, HashSet<Policy>>,
        destination_policy: Policy,
    ) -> bool {
        // Merge local and remote source policies, ignoring the node
        // TODO: implement node based policies
        for source_policy in source_policies.values().flatten() {
            // Integrity check: Source integrity must be greater than or equal to destination integrity
            if source_policy.integrity < destination_policy.integrity {
                return false;
            }

            // Confidentiality check: Secret data cannot flow to public destinations
            if source_policy.confidentiality == ConfidentialityPolicy::Secret
                && destination_policy.confidentiality == ConfidentialityPolicy::Public
            {
                return false;
            }
        }

        true
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
                ComplianceRequest::CheckCompliance {
                    source_policies,
                    destination_policy,
                } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance] CheckCompliance: source_policies: {:?}, destination_policy: {:?}",
                        source_policies, destination_policy
                    );
                    if this
                        .compliance_check(source_policies, destination_policy)
                        .await
                    {
                        Ok(ComplianceResponse::Grant)
                    } else {
                        Err(TraceabilityError::DirectPolicyViolation)
                    }
                }
                ComplianceRequest::GetPolicies(resources) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[compliance] GetPolicies: resources: {:?}", resources);
                    let policies = this.get_policies(resources).await;
                    Ok(ComplianceResponse::Policies(policies))
                }
                ComplianceRequest::SetPolicy { resource, policy } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[compliance] SetPolicy: resource: {:?}, policy: {:?}",
                        resource, policy
                    );
                    this.set_policy(resource, policy).await;
                    Ok(ComplianceResponse::PolicyUpdated)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::traceability::naming::Resource;

    use super::*;

    #[tokio::test]
    async fn unit_compliance_set_policy_basic() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();
        let process = Resource::new_process(0);

        let policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 5,
        };

        // First time setting policy should return None
        assert!(
            compliance
                .set_policy(process.clone(), policy.clone())
                .await
                .is_none()
        );

        let new_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 5,
        };

        // Second time setting policy should return the old policy
        assert_eq!(
            compliance.set_policy(process, new_policy).await,
            Some(policy)
        );
    }

    #[tokio::test]
    async fn unit_compliance_check_integrity_pass() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();

        let source_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 5,
        };

        let dest_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 3,
        };

        assert!(
            compliance
                .compliance_check(
                    HashMap::from([(String::new(), HashSet::from([source_policy]))]),
                    dest_policy
                )
                .await
        );
    }

    #[tokio::test]
    async fn unit_compliance_check_integrity_fail() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();

        let source_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 3,
        };

        let dest_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 5,
        };

        assert!(
            !compliance
                .compliance_check(
                    HashMap::from([(String::new(), HashSet::from([source_policy]))]),
                    dest_policy
                )
                .await
        );
    }

    #[tokio::test]
    async fn unit_compliance_check_confidentiality_pass() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();

        let source_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 5,
        };

        let dest_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 3,
        };

        assert!(
            compliance
                .compliance_check(
                    HashMap::from([(String::new(), HashSet::from([source_policy]))]),
                    dest_policy
                )
                .await
        );
    }

    #[tokio::test]
    async fn unit_compliance_check_confidentiality_fail() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();

        let source_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 5,
        };

        let dest_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 3,
        };

        assert!(
            !compliance
                .compliance_check(
                    HashMap::from([(String::new(), HashSet::from([source_policy]))]),
                    dest_policy
                )
                .await
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
                .compliance_check(
                    HashMap::from([
                        (
                            String::new(),
                            HashSet::from([Policy::default(), Policy::default()])
                        ),
                        ("10.0.0.1".to_string(), HashSet::from([Policy::default()]))
                    ]),
                    Policy::default()
                )
                .await
        );
    }

    #[tokio::test]
    async fn unit_compliance_check_mixed_default_explicit() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();

        let dest_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 2,
        };

        // Source uses default (integrity 0), destination has integrity 2
        assert!(
            !compliance
                .compliance_check(
                    HashMap::from([
                        (String::new(), HashSet::from([Policy::default()])),
                        ("10.0.0.1".to_string(), HashSet::from([Policy::default()]))
                    ]),
                    dest_policy
                )
                .await
        );
    }

    #[tokio::test]
    async fn unit_compliance_service_request_grant() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut compliance = ComplianceService::default();

        let source_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 5,
        };

        let dest_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 3,
        };

        let request = ComplianceRequest::CheckCompliance {
            source_policies: HashMap::from([(String::new(), HashSet::from([source_policy]))]),
            destination_policy: dest_policy,
        };

        assert_eq!(
            compliance.call(request).await.unwrap(),
            ComplianceResponse::Grant
        );
    }

    #[tokio::test]
    async fn unit_compliance_service_request_deny() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut compliance = ComplianceService::default();

        let source_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 5,
        };

        let dest_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 3,
        };

        let request = ComplianceRequest::CheckCompliance {
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
        };

        let medium_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 5,
        };

        let low_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 1,
        };

        // High -> Medium: Should pass (integrity 10 >= 5, secret -> public is blocked but this is reverse)
        let request1 = ComplianceRequest::CheckCompliance {
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
        let request2 = ComplianceRequest::CheckCompliance {
            source_policies: HashMap::from([(String::new(), HashSet::from([medium_policy]))]),
            destination_policy: low_policy.clone(),
        };
        assert_eq!(
            compliance.call(request2).await.unwrap(),
            ComplianceResponse::Grant
        );

        // Low -> High: Should fail (integrity 1 < 10)
        let request3 = ComplianceRequest::CheckCompliance {
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
        let process = Resource::new_process(0);
        let file = Resource::new_file("/tmp/test".to_string());

        assert_eq!(
            compliance
                .get_policies(HashSet::from([process, file]))
                .await,
            HashSet::from([Policy::default()])
        );
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_single_resource() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();
        let process = Resource::new_process(0);

        let policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 7,
        };

        compliance.set_policy(process.clone(), policy.clone()).await;

        assert_eq!(
            compliance.get_policies(HashSet::from([process])).await,
            HashSet::from([policy])
        );
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_multiple_resources() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();
        let process = Resource::new_process(0);
        let file = Resource::new_file("/tmp/test".to_string());

        let process_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 5,
        };

        let file_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 3,
        };

        compliance
            .set_policy(process.clone(), process_policy.clone())
            .await;
        compliance
            .set_policy(file.clone(), file_policy.clone())
            .await;

        assert_eq!(
            compliance
                .get_policies(HashSet::from([process, file]))
                .await,
            HashSet::from([process_policy, file_policy])
        );
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_mixed_existing_default() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();
        let process = Resource::new_process(0);
        let file = Resource::new_file("/tmp/test".to_string());

        let process_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 8,
        };

        // Set policy only for process, not for file
        compliance
            .set_policy(process.clone(), process_policy.clone())
            .await;

        assert_eq!(
            compliance
                .get_policies(HashSet::from([process, file]))
                .await,
            HashSet::from([process_policy, Policy::default()])
        );
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_empty_request() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();
        assert_eq!(compliance.get_policies(HashSet::new()).await.len(), 0);
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_after_update() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let compliance = ComplianceService::default();
        let process = Resource::new_process(0);

        let initial_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 2,
        };

        let updated_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 9,
        };

        // Set initial policy
        compliance
            .set_policy(process.clone(), initial_policy.clone())
            .await;

        // Verify initial policy
        assert_eq!(
            compliance
                .get_policies(HashSet::from([process.clone()]))
                .await,
            HashSet::from([initial_policy])
        );

        // Update policy
        compliance
            .set_policy(process.clone(), updated_policy.clone())
            .await;

        // Verify updated policy
        assert_eq!(
            compliance.get_policies(HashSet::from([process])).await,
            HashSet::from([updated_policy])
        );
    }
}
