use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::collections::{HashMap, HashSet};
use tower::Service;

use trace2e_middleware::traceability::{
    api::{ComplianceRequest, SequencerRequest, ProvenanceRequest},
    core::{
        compliance::{ComplianceService, Policy, ConfidentialityPolicy},
        sequencer::SequencerService,
        provenance::ProvenanceService,
    },
    naming::Resource,
};

// Helper functions for creating test data
fn create_test_process(id: i32) -> Resource {
    Resource::new_process_mock(id)
}

fn create_test_file(path: &str) -> Resource {
    Resource::new_file(path.to_string())
}

fn create_test_stream(local: &str, peer: &str) -> Resource {
    Resource::new_stream(local.to_string(), peer.to_string())
}

fn create_test_policy(confidentiality: ConfidentialityPolicy, integrity: u32, deleted: bool) -> Policy {
    Policy {
        confidentiality,
        integrity,
        deleted,
    }
}

// ComplianceService Benchmarks
fn bench_compliance_set_policy(c: &mut Criterion) {
    c.bench_function("compliance_set_policy", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut compliance = ComplianceService::default();
            let process = create_test_process(0);
            let policy = create_test_policy(ConfidentialityPolicy::Secret, 5, false);
            
            let _ = black_box(compliance.call(ComplianceRequest::SetPolicy { resource: process, policy }).await);
        });
    });
}

fn bench_compliance_get_policies_single(c: &mut Criterion) {
    c.bench_function("compliance_get_policies_single", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut compliance = ComplianceService::default();
            let process = create_test_process(0);
            let resources = HashSet::from([process]);
            
            let _ = black_box(compliance.call(ComplianceRequest::GetPolicies(resources)).await);
        });
    });
}

fn bench_compliance_get_policies_multiple(c: &mut Criterion) {
    c.bench_function("compliance_get_policies_multiple", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut compliance = ComplianceService::default();
            let resources = HashSet::from([
                create_test_process(0),
                create_test_process(1),
                create_test_file("/tmp/test1"),
                create_test_file("/tmp/test2"),
                create_test_file("/tmp/test3"),
            ]);
            
            let _ = black_box(compliance.call(ComplianceRequest::GetPolicies(resources)).await);
        });
    });
}

fn bench_compliance_check_pass(c: &mut Criterion) {
    c.bench_function("compliance_check_pass", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut compliance = ComplianceService::default();
            let source_policy = create_test_policy(ConfidentialityPolicy::Public, 5, false);
            let dest_policy = create_test_policy(ConfidentialityPolicy::Public, 3, false);
            let source_policies = HashMap::from([
                (String::new(), HashSet::from([source_policy]))
            ]);
            
            let _ = black_box(compliance.call(ComplianceRequest::CheckCompliance { source_policies, destination_policy: dest_policy }).await);
        });
    });
}

fn bench_compliance_check_fail(c: &mut Criterion) {
    c.bench_function("compliance_check_fail", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut compliance = ComplianceService::default();
            let source_policy = create_test_policy(ConfidentialityPolicy::Secret, 5, false);
            let dest_policy = create_test_policy(ConfidentialityPolicy::Public, 3, false);
            let source_policies = HashMap::from([
                (String::new(), HashSet::from([source_policy]))
            ]);
            
            let _ = black_box(compliance.call(ComplianceRequest::CheckCompliance { source_policies, destination_policy: dest_policy }).await);
        });
    });
}

fn bench_compliance_check_complex(c: &mut Criterion) {
    c.bench_function("compliance_check_complex", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut compliance = ComplianceService::default();
            let high_policy = create_test_policy(ConfidentialityPolicy::Secret, 10, false);
            let medium_policy = create_test_policy(ConfidentialityPolicy::Public, 5, false);
            let low_policy = create_test_policy(ConfidentialityPolicy::Public, 1, false);
            
            let source_policies = HashMap::from([
                (String::new(), HashSet::from([Policy::default()])),
                ("10.0.0.1".to_string(), HashSet::from([high_policy, medium_policy])),
                ("10.0.0.2".to_string(), HashSet::from([low_policy])),
            ]);
            let dest_policy = create_test_policy(ConfidentialityPolicy::Public, 3, false);
            
            let _ = black_box(compliance.call(ComplianceRequest::CheckCompliance { source_policies, destination_policy: dest_policy }).await);
        });
    });
}

// SequencerService Benchmarks  
fn bench_sequencer_make_flow_success(c: &mut Criterion) {
    c.bench_function("sequencer_make_flow_success", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut sequencer = SequencerService::default();
            let process = create_test_process(0);
            let file = create_test_file("/tmp/test");
            
            let _ = black_box(sequencer.call(SequencerRequest::ReserveFlow { source: process, destination: file }).await);
        });
    });
}

fn bench_sequencer_make_flow_conflict(c: &mut Criterion) {
    c.bench_function("sequencer_make_flow_conflict", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut sequencer = SequencerService::default();
            let process = create_test_process(0);
            let file = create_test_file("/tmp/test");
            
            // Create initial flow
            let _ = sequencer.call(SequencerRequest::ReserveFlow { source: process.clone(), destination: file.clone() }).await;
            
            // Try to create conflicting flow
            let _ = black_box(sequencer.call(SequencerRequest::ReserveFlow { source: process, destination: file }).await.unwrap_err());
        });
    });
}

fn bench_sequencer_drop_flow(c: &mut Criterion) {
    c.bench_function("sequencer_drop_flow", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut sequencer = SequencerService::default();
            let process = create_test_process(0);
            let file = create_test_file("/tmp/test");
            
            // Create flow first
            let _ = sequencer.call(SequencerRequest::ReserveFlow { source: process, destination: file.clone() }).await;
            
            let _ = black_box(sequencer.call(SequencerRequest::ReleaseFlow { destination: file }).await);
        });
    });
}

fn bench_sequencer_multiple_readers(c: &mut Criterion) {
    c.bench_function("sequencer_multiple_readers", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut sequencer = SequencerService::default();
            let process = create_test_process(0);
            let files = vec![
                create_test_file("/tmp/test1"),
                create_test_file("/tmp/test2"),
                create_test_file("/tmp/test3"),
                create_test_file("/tmp/test4"),
                create_test_file("/tmp/test5"),
            ];
            
            // Create multiple reader flows
            for file in &files {
                let file = file.clone();
                let process = process.clone();
                let _ = black_box(sequencer.call(SequencerRequest::ReserveFlow { source: process, destination: file }).await);
            }
            
            // Drop flows
            for file in &files {
                let file = file.clone();
                let _ = black_box(sequencer.call(SequencerRequest::ReleaseFlow { destination: file }).await);
            }
        });
    });
}

// ProvenanceService Benchmarks
fn bench_provenance_update_simple(c: &mut Criterion) {
    c.bench_function("provenance_update_simple", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut provenance = ProvenanceService::default();
            let process = create_test_process(0);
            let file = create_test_file("/tmp/test");
            
            let _ = black_box(provenance.call(ProvenanceRequest::UpdateProvenance { source: file, destination: process }).await);
        });
    });
}

fn bench_provenance_update_circular(c: &mut Criterion) {
    c.bench_function("provenance_update_circular", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut provenance = ProvenanceService::default();
            let process = create_test_process(0);
            let process_clone = process.clone();
            let file = create_test_file("/tmp/test");
            let file_clone = file.clone();
            
            let _ = black_box(provenance.call(ProvenanceRequest::UpdateProvenance { source: process_clone, destination: file_clone }).await);
            let _ = black_box(provenance.call(ProvenanceRequest::UpdateProvenance { source: file, destination: process}).await);
        });
    });
}

fn bench_provenance_update_stream(c: &mut Criterion) {
    c.bench_function("provenance_update_stream", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut provenance = ProvenanceService::default();
            let process0 = create_test_process(0);
            let process1 = create_test_process(1);
            let stream = create_test_stream("127.0.0.1:8080", "127.0.0.1:8081");
            let stream_clone = stream.clone();
            
            let _ = black_box(provenance.call(ProvenanceRequest::UpdateProvenance { source: process0, destination: stream_clone }).await);
            let _ = black_box(provenance.call(ProvenanceRequest::UpdateProvenance { source: stream, destination: process1 }).await);
        });
    });
}

fn bench_provenance_update_raw_multiple_nodes(c: &mut Criterion) {
    c.bench_function("provenance_update_raw_multiple_nodes", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut provenance = ProvenanceService::default();
            let process0 = create_test_process(0);
            let process0_clone = process0.clone();
            let process1 = create_test_process(1);
            let file0 = create_test_file("/tmp/test0");
            
            let source_prov = HashMap::from([
                ("10.0.0.1".to_string(), HashSet::from([process0.clone()])),
                ("10.0.0.2".to_string(), HashSet::from([process0.clone()])),
            ]);
            
            let _ = black_box(provenance.call(ProvenanceRequest::UpdateProvenanceRaw { source_prov, destination: process0_clone }).await);
            
            let source_prov2 = HashMap::from([
                ("10.0.0.1".to_string(), HashSet::from([process1.clone()])),
                ("10.0.0.2".to_string(), HashSet::from([file0.clone(), process1.clone()])),
            ]);
            
            let _ = black_box(provenance.call(ProvenanceRequest::UpdateProvenanceRaw { source_prov: source_prov2, destination: process0 }).await);
        });
    });
}

fn bench_provenance_get_prov_empty(c: &mut Criterion) {
    c.bench_function("provenance_get_prov_empty", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut provenance = ProvenanceService::default();
            let process = create_test_process(0);
            
            let _ = black_box(provenance.call(ProvenanceRequest::GetReferences(process)).await);
        });
    });
}

fn bench_provenance_get_prov_populated(c: &mut Criterion) {
    c.bench_function("provenance_get_prov_populated", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut provenance = ProvenanceService::default();
            let process = create_test_process(0);
            let files = vec![
                create_test_file("/tmp/test1"),
                create_test_file("/tmp/test2"),
                create_test_file("/tmp/test3"),
            ];
            
            // Build up provenance
            for file in &files {
                let _ = provenance.call(ProvenanceRequest::UpdateProvenance { source: file.clone(), destination: process.clone() }).await;
            }
            
            let _ = black_box(provenance.call(ProvenanceRequest::GetReferences(process.clone())).await);
        });
    });
}

criterion_group!(
    compliance_benches,
    bench_compliance_set_policy,
    bench_compliance_get_policies_single,
    bench_compliance_get_policies_multiple,
    bench_compliance_check_pass,
    bench_compliance_check_fail,
    bench_compliance_check_complex,
);

criterion_group!(
    sequencer_benches,
    bench_sequencer_make_flow_success,
    bench_sequencer_make_flow_conflict,
    bench_sequencer_drop_flow,
    bench_sequencer_multiple_readers,
);

criterion_group!(
    provenance_benches,
    bench_provenance_update_simple,
    bench_provenance_update_circular,
    bench_provenance_update_stream,
    bench_provenance_update_raw_multiple_nodes,
    bench_provenance_get_prov_empty,
    bench_provenance_get_prov_populated,
);

criterion_main!(
    compliance_benches,
    sequencer_benches,
    provenance_benches
);
