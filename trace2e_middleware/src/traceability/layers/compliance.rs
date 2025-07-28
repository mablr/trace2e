use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use tokio::sync::Mutex;
use tower::Service;

use crate::traceability::{
    api::{ComplianceRequest, ComplianceResponse},
    error::TraceabilityError,
    naming::Identifier,
};

#[derive(Default, PartialEq, Debug, Clone, Eq)]
pub enum ConfidentialityPolicy {
    Secret,
    #[default]
    Public,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct Policy {
    pub confidentiality: ConfidentialityPolicy,
    pub integrity: u32,
}

#[derive(Default, Clone)]
pub struct ComplianceService {
    node_id: String,
    policies: Arc<Mutex<HashMap<Identifier, Policy>>>,
}

impl ComplianceService {
    pub fn new(node_id: String) -> Self {
        Self {
            node_id,
            policies: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get the policies for a specific resource
    /// Returns the default policy if the resource is not found
    async fn get_policies(&self, ids: HashSet<Identifier>) -> HashMap<Identifier, Policy> {
        let policies = self.policies.lock().await;
        let mut policies_map = HashMap::new();
        for id in ids {
            // If the resource is local, get the policy from the local policies
            if id.node == self.node_id {
                policies_map.insert(id.clone(), policies.get(&id).cloned().unwrap_or_default());
            }
        }
        policies_map
    }

    /// Set the policy for a specific resource
    /// Returns the old policy if the resource is already set
    async fn set_policy(&self, id: Identifier, policy: Policy) -> Option<Policy> {
        let mut policies = self.policies.lock().await;
        policies.insert(id, policy)
    }

    /// Check if a flow from source to destination is compliant with policies
    /// Returns true if the flow is compliant, false otherwise
    async fn local_flow_check(&self, source: Identifier, destination: Identifier) -> bool {
        let policies = self.policies.lock().await;

        let default_policy = Policy::default();
        let source_policy = policies.get(&source).unwrap_or(&default_policy);
        let destination_policy = policies.get(&destination).unwrap_or(&default_policy);

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

    fn call(&mut self, req: ComplianceRequest) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            match req.clone() {
                ComplianceRequest::CheckCompliance {
                    source,
                    destination,
                } => {
                    if destination.node != this.node_id || source.node != this.node_id {
                        todo!("implement distributed flow check")
                    }

                    if this.local_flow_check(source, destination).await {
                        Ok(ComplianceResponse::Grant)
                    } else {
                        Err(TraceabilityError::DirectPolicyViolation)
                    }
                }
                ComplianceRequest::GetPolicies { ids } => {
                    let policies = this.get_policies(ids).await;
                    Ok(ComplianceResponse::Policies(policies))
                }
                ComplianceRequest::SetPolicy { id, policy } => {
                    this.set_policy(id, policy).await;
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
        let compliance = ComplianceService::default();
        let process = Identifier::new(String::default(), Resource::new_process(0));

        let policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 5,
        };

        let result = compliance.set_policy(process.clone(), policy).await;
        assert!(result.is_none()); // First time setting policy should return None
    }

    #[tokio::test]
    async fn unit_compliance_set_policy_overwrite() {
        let compliance = ComplianceService::default();
        let process = Identifier::new(String::default(), Resource::new_process(0));

        let policy1 = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 5,
        };

        let policy2 = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 3,
        };

        compliance.set_policy(process.clone(), policy1).await;
        let result = compliance.set_policy(process.clone(), policy2).await;

        assert!(result.is_some());
        let old_policy = result.unwrap();
        assert_eq!(old_policy.confidentiality, ConfidentialityPolicy::Secret);
        assert_eq!(old_policy.integrity, 5);
    }

    #[tokio::test]
    async fn unit_compliance_check_integrity_pass() {
        let compliance = ComplianceService::default();
        let source = Identifier::new(String::default(), Resource::new_process(0));
        let destination = Identifier::new(
            String::default(),
            Resource::new_file("/tmp/test".to_string()),
        );

        let source_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 5,
        };

        let dest_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 3,
        };

        compliance.set_policy(source.clone(), source_policy).await;
        compliance
            .set_policy(destination.clone(), dest_policy)
            .await;

        assert!(compliance.local_flow_check(source, destination).await);
    }

    #[tokio::test]
    async fn unit_compliance_check_integrity_fail() {
        let compliance = ComplianceService::default();
        let source = Identifier::new(String::default(), Resource::new_process(0));
        let destination = Identifier::new(
            String::default(),
            Resource::new_file("/tmp/test".to_string()),
        );

        let source_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 3,
        };

        let dest_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 5,
        };

        compliance.set_policy(source.clone(), source_policy).await;
        compliance
            .set_policy(destination.clone(), dest_policy)
            .await;

        assert!(!compliance.local_flow_check(source, destination).await);
    }

    #[tokio::test]
    async fn unit_compliance_check_confidentiality_pass() {
        let compliance = ComplianceService::default();
        let source = Identifier::new(String::default(), Resource::new_process(0));
        let destination = Identifier::new(
            String::default(),
            Resource::new_file("/tmp/test".to_string()),
        );

        let source_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 5,
        };

        let dest_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 3,
        };

        compliance.set_policy(source.clone(), source_policy).await;
        compliance
            .set_policy(destination.clone(), dest_policy)
            .await;

        assert!(compliance.local_flow_check(source, destination).await);
    }

    #[tokio::test]
    async fn unit_compliance_check_confidentiality_fail() {
        let compliance = ComplianceService::default();
        let source = Identifier::new(String::default(), Resource::new_process(0));
        let destination = Identifier::new(
            String::default(),
            Resource::new_file("/tmp/test".to_string()),
        );

        let source_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 5,
        };

        let dest_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 3,
        };

        compliance.set_policy(source.clone(), source_policy).await;
        compliance
            .set_policy(destination.clone(), dest_policy)
            .await;

        assert!(!compliance.local_flow_check(source, destination).await);
    }

    #[tokio::test]
    async fn unit_compliance_check_default_policies() {
        let compliance = ComplianceService::default();
        let node_id = "test".to_string();
        let source = Identifier::new(node_id.clone(), Resource::new_process(0));
        let destination =
            Identifier::new(node_id.clone(), Resource::new_file("/tmp/test".to_string()));

        // Both should use default policies (Public, integrity 0)
        assert!(compliance.local_flow_check(source, destination).await);
    }

    #[tokio::test]
    async fn unit_compliance_check_mixed_default_explicit() {
        let compliance = ComplianceService::default();
        let source = Identifier::new(String::default(), Resource::new_process(0));
        let destination = Identifier::new(
            String::default(),
            Resource::new_file("/tmp/test".to_string()),
        );

        let dest_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 2,
        };

        compliance
            .set_policy(destination.clone(), dest_policy)
            .await;

        // Source uses default (integrity 0), destination has integrity 2
        assert!(!compliance.local_flow_check(source, destination).await);
    }

    #[tokio::test]
    async fn unit_compliance_service_request_grant() {
        let mut compliance = ComplianceService::default();
        let source = Identifier::new(String::default(), Resource::new_process(0));
        let destination = Identifier::new(
            String::default(),
            Resource::new_file("/tmp/test".to_string()),
        );

        let source_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 5,
        };

        let dest_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 3,
        };

        compliance.set_policy(source.clone(), source_policy).await;
        compliance
            .set_policy(destination.clone(), dest_policy)
            .await;

        let request = ComplianceRequest::CheckCompliance {
            source,
            destination,
        };
        let response = compliance.call(request).await.unwrap();

        assert_eq!(response, ComplianceResponse::Grant);
    }

    #[tokio::test]
    async fn unit_compliance_service_request_deny() {
        let mut compliance = ComplianceService::default();
        let source = Identifier::new(String::default(), Resource::new_process(0));
        let destination = Identifier::new(
            String::default(),
            Resource::new_file("/tmp/test".to_string()),
        );

        let source_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 5,
        };

        let dest_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 3,
        };

        compliance.set_policy(source.clone(), source_policy).await;
        compliance
            .set_policy(destination.clone(), dest_policy)
            .await;

        let request = ComplianceRequest::CheckCompliance {
            source,
            destination,
        };
        let error = compliance.call(request).await.unwrap_err();

        assert_eq!(error, TraceabilityError::DirectPolicyViolation);
    }

    #[tokio::test]
    async fn unit_compliance_service_report_ack() {
        let mut compliance = ComplianceService::default();
        let source = Identifier::new(String::default(), Resource::new_process(0));
        let destination = Identifier::new(
            String::default(),
            Resource::new_file("/tmp/test".to_string()),
        );

        let request = ComplianceRequest::CheckCompliance {
            source,
            destination,
        };
        let response = compliance.call(request).await.unwrap();

        assert_eq!(response, ComplianceResponse::Grant);
    }

    #[tokio::test]
    async fn unit_compliance_service_complex_policy_scenario() {
        let mut compliance = ComplianceService::default();

        // Create multiple resources with different security levels
        let high_security_process = Identifier::new(String::default(), Resource::new_process(1));
        let medium_security_file = Identifier::new(
            String::default(),
            Resource::new_file("/tmp/medium".to_string()),
        );
        let low_security_file = Identifier::new(
            String::default(),
            Resource::new_file("/tmp/low".to_string()),
        );

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

        compliance
            .set_policy(high_security_process.clone(), high_policy)
            .await;
        compliance
            .set_policy(medium_security_file.clone(), medium_policy)
            .await;
        compliance
            .set_policy(low_security_file.clone(), low_policy)
            .await;

        // High -> Medium: Should pass (integrity 10 >= 5, secret -> public is blocked but this is reverse)
        let request1 = ComplianceRequest::CheckCompliance {
            source: high_security_process.clone(),
            destination: medium_security_file.clone(),
        };
        let error1 = compliance.call(request1).await.unwrap_err();
        assert_eq!(error1, TraceabilityError::DirectPolicyViolation); // Secret -> Public fails

        // Medium -> Low: Should pass (integrity 5 >= 1, public -> public)
        let request2 = ComplianceRequest::CheckCompliance {
            source: medium_security_file.clone(),
            destination: low_security_file.clone(),
        };
        let response2 = compliance.call(request2).await.unwrap();
        assert_eq!(response2, ComplianceResponse::Grant);

        // Low -> High: Should fail (integrity 1 < 10)
        let request3 = ComplianceRequest::CheckCompliance {
            source: low_security_file.clone(),
            destination: high_security_process.clone(),
        };
        let error3 = compliance.call(request3).await.unwrap_err();
        assert_eq!(error3, TraceabilityError::DirectPolicyViolation);
    }

    #[tokio::test]
    #[should_panic] // todo : implement distributed flow check
    async fn unit_compliance_service_layer_distributed_flow_check() {
        let remote_node_id = "remote".to_string();
        let local_node_id = "local".to_string();
        let mut compliance_service = ComplianceService::new(local_node_id.clone());
        let source = Identifier::new(remote_node_id.clone(), Resource::new_process(0));
        let destination = Identifier::new(
            local_node_id.clone(),
            Resource::new_file("/tmp/test".to_string()),
        );

        let request = ComplianceRequest::CheckCompliance {
            source: source.clone(),
            destination: destination.clone(),
        };
        let response = compliance_service.call(request).await.unwrap();
        assert_eq!(response, ComplianceResponse::Grant);
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_empty() {
        let compliance = ComplianceService::default();
        let process = Identifier::new(String::default(), Resource::new_process(0));
        let file = Identifier::new(
            String::default(),
            Resource::new_file("/tmp/test".to_string()),
        );

        let mut ids = HashSet::new();
        ids.insert(process.clone());
        ids.insert(file.clone());

        let policies = compliance.get_policies(ids).await;

        assert_eq!(policies.len(), 2);
        assert_eq!(policies.get(&process).unwrap(), &Policy::default());
        assert_eq!(policies.get(&file).unwrap(), &Policy::default());
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_single_resource() {
        let compliance = ComplianceService::default();
        let process = Identifier::new(String::default(), Resource::new_process(0));

        let policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 7,
        };

        compliance.set_policy(process.clone(), policy.clone()).await;

        let mut ids = HashSet::new();
        ids.insert(process.clone());

        let policies = compliance.get_policies(ids).await;

        assert_eq!(policies.len(), 1);
        assert_eq!(policies.get(&process).unwrap(), &policy);
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_multiple_resources() {
        let compliance = ComplianceService::default();
        let process = Identifier::new(String::default(), Resource::new_process(0));
        let file = Identifier::new(
            String::default(),
            Resource::new_file("/tmp/test".to_string()),
        );

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

        let mut ids = HashSet::new();
        ids.insert(process.clone());
        ids.insert(file.clone());

        let policies = compliance.get_policies(ids).await;

        assert_eq!(policies.len(), 2);
        assert_eq!(policies.get(&process).unwrap(), &process_policy);
        assert_eq!(policies.get(&file).unwrap(), &file_policy);
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_mixed_existing_default() {
        let compliance = ComplianceService::default();
        let process = Identifier::new(String::default(), Resource::new_process(0));
        let file = Identifier::new(
            String::default(),
            Resource::new_file("/tmp/test".to_string()),
        );

        let process_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 8,
        };

        // Set policy only for process, not for file
        compliance
            .set_policy(process.clone(), process_policy.clone())
            .await;

        let mut ids = HashSet::new();
        ids.insert(process.clone());
        ids.insert(file.clone());

        let policies = compliance.get_policies(ids).await;

        assert_eq!(policies.len(), 2);
        assert_eq!(policies.get(&process).unwrap(), &process_policy);
        assert_eq!(policies.get(&file).unwrap(), &Policy::default());
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_empty_request() {
        let compliance = ComplianceService::default();
        let ids = HashSet::new();

        let policies = compliance.get_policies(ids).await;

        assert_eq!(policies.len(), 0);
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_after_update() {
        let compliance = ComplianceService::default();
        let process = Identifier::new(String::default(), Resource::new_process(0));

        let initial_policy = Policy {
            confidentiality: ConfidentialityPolicy::Public,
            integrity: 2,
        };

        let updated_policy = Policy {
            confidentiality: ConfidentialityPolicy::Secret,
            integrity: 9,
        };

        // Set initial policy
        compliance.set_policy(process.clone(), initial_policy).await;

        let mut ids = HashSet::new();
        ids.insert(process.clone());

        // Verify initial policy
        let policies = compliance.get_policies(ids.clone()).await;
        assert_eq!(policies.len(), 1);
        assert_eq!(policies.get(&process).unwrap().integrity, 2);
        assert_eq!(
            policies.get(&process).unwrap().confidentiality,
            ConfidentialityPolicy::Public
        );

        // Update policy
        compliance
            .set_policy(process.clone(), updated_policy.clone())
            .await;

        // Verify updated policy
        let policies = compliance.get_policies(ids).await;
        assert_eq!(policies.len(), 1);
        assert_eq!(policies.get(&process).unwrap(), &updated_policy);
    }

    #[tokio::test]
    async fn unit_compliance_get_policies_filter_local_resources() {
        let compliance = ComplianceService::default();
        let process = Identifier::new(String::default(), Resource::new_process(0));
        let file = Identifier::new(
            "remote".to_string(),
            Resource::new_file("/tmp/test".to_string()),
        );

        let mut ids = HashSet::new();
        ids.insert(process.clone());
        ids.insert(file.clone());

        let policies = compliance.get_policies(ids).await;

        assert_eq!(policies.len(), 1);
        assert_eq!(policies.get(&process).is_some(), true);
        assert_eq!(policies.get(&file).is_none(), true);
    }
}
