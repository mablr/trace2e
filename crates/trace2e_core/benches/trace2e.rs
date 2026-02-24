use std::collections::HashSet;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use tower::Service;
use trace2e_core::{
    traceability::{
        api::{
            m2m::M2mApiService,
            p2m::P2mApiService,
            types::{P2mRequest, P2mResponse},
        },
        infrastructure::naming::{LocalizedResource, Resource},
        services::{
            compliance::ComplianceService,
            consent::ConsentService,
            provenance::ProvenanceService,
            sequencer::{SequencerService, WaitingQueueService},
        },
    },
    transport::{loopback::M2mLoopback, nop::M2mNop},
};

const PROVENANCE_SIZES: [usize; 4] = [0, 10, 100, 1000];

fn build_service(
    n: usize,
) -> P2mApiService<SequencerService, ProvenanceService, ComplianceService, M2mNop> {
    let node_id = String::new();
    let provenance = ProvenanceService::new(node_id.clone());
    let file = Resource::new_file("/tmp/bench".to_string());
    let process_mock = Resource::new_process_mock(1);

    let refs: HashSet<LocalizedResource> = (0..n)
        .map(|i| {
            LocalizedResource::new(node_id.clone(), Resource::new_file(format!("/tmp/ref_{i}")))
        })
        .collect();
    provenance.set_references(file.clone(), refs);

    P2mApiService::new(
        SequencerService::default(),
        provenance,
        ComplianceService::default(),
        M2mNop,
    )
    .with_enrolled_resource(1, 3, process_mock, file)
}

fn local_io(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("local_io");

    for n in PROVENANCE_SIZES {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || build_service(n),
                |mut svc| {
                    rt.block_on(async {
                        let P2mResponse::Grant(grant_id) = svc
                            .call(P2mRequest::IoRequest { pid: 1, fd: 3, output: false })
                            .await
                            .unwrap()
                        else {
                            panic!("Expected Grant");
                        };
                        svc.call(P2mRequest::IoReport { pid: 1, fd: 3, grant_id, result: true })
                            .await
                            .unwrap();
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn local_io_request(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("local_io_request");

    for n in PROVENANCE_SIZES {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || build_service(n),
                |mut svc| {
                    rt.block_on(async {
                        svc.call(P2mRequest::IoRequest { pid: 1, fd: 3, output: false })
                            .await
                            .unwrap();
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn build_distributed_services(
    n: usize,
) -> (
    P2mApiService<
        WaitingQueueService<SequencerService>,
        ProvenanceService,
        ComplianceService,
        M2mLoopback,
    >,
    M2mLoopback,
) {
    let m2m = M2mLoopback::default();

    let node1 = "10.0.0.1".to_string();
    let node2 = "10.0.0.2".to_string();

    // Build node 2 (remote) — its M2M service handles CheckSourceCompliance
    let seq2 = WaitingQueueService::new(SequencerService::default(), None);
    let prov2 = ProvenanceService::new(node2.clone());
    let consent2 = ConsentService::new(0);
    let comp2 = ComplianceService::new(node2.clone(), consent2);
    let m2m_svc2 = M2mApiService::new(seq2, prov2, comp2);

    // Build node 1 (local) — this is the one we benchmark
    let seq1 = WaitingQueueService::new(SequencerService::default(), None);
    let prov1 = ProvenanceService::new(node1.clone());
    let consent1 = ConsentService::new(0);
    let comp1 = ComplianceService::new(node1.clone(), consent1);

    // Pre-populate the stream on node 1 with N provenance refs located on node 2
    let stream1 = Resource::new_stream("10.0.0.1:1337".into(), "10.0.0.2:1338".into());
    let refs: HashSet<LocalizedResource> = (0..n)
        .map(|i| LocalizedResource::new(node2.clone(), Resource::new_file(format!("/tmp/ref_{i}"))))
        .collect();
    prov1.set_references(stream1.clone(), refs);

    let m2m_svc1 = M2mApiService::new(seq1.clone(), prov1.clone(), comp1.clone());

    let p2m1 = P2mApiService::new(seq1, prov1, comp1, m2m.clone()).with_enrolled_resource(
        1,
        3,
        Resource::new_process_mock(1),
        stream1,
    );

    // Register both M2M services synchronously (DashMap insert under the hood)
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        m2m.register_middleware(node1, m2m_svc1).await;
        m2m.register_middleware(node2, m2m_svc2).await;
    });

    (p2m1, m2m)
}

fn distributed_io_request(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("distributed_io_request");

    for n in PROVENANCE_SIZES {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || build_distributed_services(n),
                |(mut svc, _m2m)| {
                    rt.block_on(async {
                        svc.call(P2mRequest::IoRequest { pid: 1, fd: 3, output: false })
                            .await
                            .unwrap();
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn distributed_io(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("distributed_io");

    for n in PROVENANCE_SIZES {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || build_distributed_services(n),
                |(mut svc, _m2m)| {
                    rt.block_on(async {
                        let P2mResponse::Grant(grant_id) = svc
                            .call(P2mRequest::IoRequest { pid: 1, fd: 3, output: false })
                            .await
                            .unwrap()
                        else {
                            panic!("Expected Grant");
                        };
                        svc.call(P2mRequest::IoReport { pid: 1, fd: 3, grant_id, result: true })
                            .await
                            .unwrap();
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, local_io, local_io_request, distributed_io, distributed_io_request);
criterion_main!(benches);
