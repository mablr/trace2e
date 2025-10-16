use std::collections::{HashMap, HashSet};

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use tower::Service;
use trace2e_core::{
    traceability::{
        api::{ComplianceRequest, P2mRequest, P2mResponse, ProvenanceRequest, SequencerRequest},
        infrastructure::naming::Resource,
        services::{
            compliance::{ComplianceService, ConfidentialityPolicy, DeletionPolicy, Policy},
            provenance::ProvenanceService,
            sequencer::SequencerService,
        },
    },
    transport::loopback::spawn_loopback_middlewares_with_enrolled_resources,
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

// ComplianceService Benchmarks
fn bench_compliance_set_policy(c: &mut Criterion) {
    c.bench_function("compliance_set_policy", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut compliance = ComplianceService::default();
            let process = create_test_process(0);
            let policy =
                Policy::new(ConfidentialityPolicy::Secret, 5, DeletionPolicy::NotDeleted, true, HashSet::new());

            let _ = black_box(
                compliance.call(ComplianceRequest::SetPolicy { resource: process, policy }).await,
            );
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

// SequencerService Benchmarks
fn bench_sequencer_make_flow_success(c: &mut Criterion) {
    c.bench_function("sequencer_make_flow_success", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut sequencer = SequencerService::default();
            let process = create_test_process(0);
            let file = create_test_file("/tmp/test");

            let _ = black_box(
                sequencer
                    .call(SequencerRequest::ReserveFlow { source: process, destination: file })
                    .await,
            );
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
            let _ = sequencer
                .call(SequencerRequest::ReserveFlow {
                    source: process.clone(),
                    destination: file.clone(),
                })
                .await;

            // Try to create conflicting flow
            let _ = black_box(
                sequencer
                    .call(SequencerRequest::ReserveFlow { source: process, destination: file })
                    .await
                    .unwrap_err(),
            );
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
            let _ = sequencer
                .call(SequencerRequest::ReserveFlow { source: process, destination: file.clone() })
                .await;

            let _ = black_box(
                sequencer.call(SequencerRequest::ReleaseFlow { destination: file }).await,
            );
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
                let file = file.to_owned();
                let process = process.clone();
                let _ = black_box(
                    sequencer
                        .call(SequencerRequest::ReserveFlow { source: process, destination: file })
                        .await,
                );
            }

            // Drop flows
            for file in &files {
                let file = file.to_owned();
                let _ = black_box(
                    sequencer.call(SequencerRequest::ReleaseFlow { destination: file }).await,
                );
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

            let _ = black_box(
                provenance
                    .call(ProvenanceRequest::UpdateProvenance {
                        source: file,
                        destination: process,
                    })
                    .await,
            );
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

            let _ = black_box(
                provenance
                    .call(ProvenanceRequest::UpdateProvenance {
                        source: process_clone,
                        destination: file_clone,
                    })
                    .await,
            );
            let _ = black_box(
                provenance
                    .call(ProvenanceRequest::UpdateProvenance {
                        source: file,
                        destination: process,
                    })
                    .await,
            );
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

            let _ = black_box(
                provenance
                    .call(ProvenanceRequest::UpdateProvenance {
                        source: process0,
                        destination: stream_clone,
                    })
                    .await,
            );
            let _ = black_box(
                provenance
                    .call(ProvenanceRequest::UpdateProvenance {
                        source: stream,
                        destination: process1,
                    })
                    .await,
            );
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

            let _ = black_box(
                provenance
                    .call(ProvenanceRequest::UpdateProvenanceRaw {
                        source_prov,
                        destination: process0_clone,
                    })
                    .await,
            );

            let source_prov2 = HashMap::from([
                ("10.0.0.1".to_string(), HashSet::from([process1.clone()])),
                ("10.0.0.2".to_string(), HashSet::from([file0.clone(), process1.clone()])),
            ]);

            let _ = black_box(
                provenance
                    .call(ProvenanceRequest::UpdateProvenanceRaw {
                        source_prov: source_prov2,
                        destination: process0,
                    })
                    .await,
            );
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
                let _ = provenance
                    .call(ProvenanceRequest::UpdateProvenance {
                        source: file.clone(),
                        destination: process.clone(),
                    })
                    .await;
            }

            let _ =
                black_box(provenance.call(ProvenanceRequest::GetReferences(process.clone())).await);
        });
    });
}

// Stress test for provenance with interference patterns
fn bench_provenance_stress_interference(c: &mut Criterion) {
    use criterion::BatchSize;

    let processes_count = 10;
    let files_per_process_count = 10;
    let streams_per_process_count = 10;

    c.bench_function("provenance_stress_interference_pull", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter_batched(
            async || {
                // Setup: Pre-initialize P2M service with many processes and files
                // This initialization time is NOT measured
                let mut p2m_services = spawn_loopback_middlewares_with_enrolled_resources(
                    vec!["127.0.0.1".to_string(), "127.0.0.2".to_string()],
                    processes_count,
                    files_per_process_count,
                    streams_per_process_count,
                )
                .await;
                (p2m_services.pop_front().unwrap().0, p2m_services.pop_front().unwrap().0)
            },
            |p2m_services| async move {
                let (mut p2m_service_0, mut p2m_service_1) = p2m_services.await;
                // Only this section is measured
                // Create interference patterns: multiple processes compete for same files
                for pid in 0..processes_count as i32 {
                    for fd in 0..(files_per_process_count + streams_per_process_count) as i32 {
                        // Complete I/O cycle: request -> grant -> report
                        if let Ok(P2mResponse::Grant(grant_id)) = p2m_service_0
                            .call(P2mRequest::IoRequest { pid, fd, output: (pid + fd) % 2 == 0 })
                            .await
                        {
                            // Report completion
                            let _ = p2m_service_0
                                .call(P2mRequest::IoReport { pid, fd, grant_id, result: true })
                                .await;
                        }
                        // Complete I/O cycle: request -> grant -> report
                        if let Ok(P2mResponse::Grant(grant_id)) = p2m_service_1
                            .call(P2mRequest::IoRequest { pid, fd, output: (pid + fd) % 2 == 0 })
                            .await
                        {
                            // Report completion
                            let _ = p2m_service_1
                                .call(P2mRequest::IoReport { pid, fd, grant_id, result: true })
                                .await;
                        }
                    }
                }
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("provenance_stress_interference_push", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter_batched(
            async || {
                // Setup: Pre-initialize P2M service with many processes and files
                // This initialization time is NOT measured
                let mut p2m_services = spawn_loopback_middlewares_with_enrolled_resources(
                    vec!["127.0.0.1".to_string(), "127.0.0.2".to_string()],
                    processes_count,
                    files_per_process_count,
                    streams_per_process_count,
                )
                .await;
                (p2m_services.pop_front().unwrap().0, p2m_services.pop_front().unwrap().0)
            },
            |p2m_services| async move {
                let (mut p2m_service_0, mut p2m_service_1) = p2m_services.await;
                // Only this section is measured
                // Create interference patterns: multiple processes compete for same files
                for pid in 0..processes_count as i32 {
                    for fd in 0..(files_per_process_count + streams_per_process_count) as i32 {
                        // Complete I/O cycle: request -> grant -> report
                        if let Ok(P2mResponse::Grant(grant_id)) = p2m_service_0
                            .call(P2mRequest::IoRequest { pid, fd, output: (pid + fd) % 2 == 0 })
                            .await
                        {
                            // Report completion
                            let _ = p2m_service_0
                                .call(P2mRequest::IoReport { pid, fd, grant_id, result: true })
                                .await;
                        }
                        // Complete I/O cycle: request -> grant -> report
                        if let Ok(P2mResponse::Grant(grant_id)) = p2m_service_1
                            .call(P2mRequest::IoRequest { pid, fd, output: (pid + fd) % 2 == 0 })
                            .await
                        {
                            // Report completion
                            let _ = p2m_service_1
                                .call(P2mRequest::IoReport { pid, fd, grant_id, result: true })
                                .await;
                        }
                    }
                }
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    compliance_benches,
    bench_compliance_set_policy,
    bench_compliance_get_policies_single,
    bench_compliance_get_policies_multiple,
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
    bench_provenance_stress_interference,
);

criterion_main!(compliance_benches, sequencer_benches, provenance_benches);
