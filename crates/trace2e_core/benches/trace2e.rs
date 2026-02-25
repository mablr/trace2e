use std::collections::HashSet;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use tonic::transport::Server;
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
    transport::{
        grpc::{
            M2mGrpc, M2mHandler, P2mHandler,
            proto::{self, m2m_server::M2mServer, p2m_server::P2mServer},
        },
        loopback::M2mLoopback,
        nop::M2mNop,
    },
};

const PROVENANCE_SIZES: [usize; 4] = [1, 10, 100, 1000];
const GRPC_REF_COUNT: usize = 100;

fn build_refs(node_id: &str, count: usize) -> HashSet<LocalizedResource> {
    (0..count)
        .map(|i| {
            LocalizedResource::new(
                node_id.to_string(),
                Resource::new_file(format!("/tmp/{node_id}/ref_{i}")),
            )
        })
        .collect()
}

fn build_service(
    n: usize,
) -> P2mApiService<SequencerService, ProvenanceService, ComplianceService, M2mNop> {
    let node_id = String::new();
    let provenance = ProvenanceService::new(node_id.clone());
    let file = Resource::new_file("/tmp/bench".to_string());
    let process_mock = Resource::new_process_mock(1);

    let refs = build_refs(&node_id, n);
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
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_resources", n)),
            &n,
            |b, &n| {
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
                            svc.call(P2mRequest::IoReport {
                                pid: 1,
                                fd: 3,
                                grant_id,
                                result: true,
                            })
                            .await
                            .unwrap();
                        })
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

const TOTAL_DISTRIBUTED_REFS: usize = 100;
const NODE_COUNTS: [usize; 4] = [1, 2, 5, 10];

fn build_distributed_services(
    num_nodes: usize,
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

    // Build remote nodes (10.0.0.2 .. 10.0.0.{num_nodes+1}), each handling CheckSourceCompliance
    let remote_ips: Vec<String> = (2..=num_nodes + 1).map(|i| format!("10.0.0.{i}")).collect();

    let refs: HashSet<LocalizedResource> = remote_ips
        .iter()
        .flat_map(|ip| build_refs(ip, TOTAL_DISTRIBUTED_REFS / num_nodes))
        .collect();

    // Build node 1 (local) — this is the one we benchmark
    let seq1 = WaitingQueueService::new(SequencerService::default(), None);
    let prov1 = ProvenanceService::new(node1.clone());
    let consent1 = ConsentService::default();
    let comp1 = ComplianceService::new(node1.clone(), consent1);

    // Use a stream whose peer is on 10.0.0.2 (arbitrary — the compliance routing
    // is driven by the node_ids in the provenance refs, not the stream peer)
    let stream1 = Resource::new_stream("10.0.0.1:1337".into(), "10.0.0.2:1338".into());
    prov1.set_references(stream1.clone(), refs);

    let m2m_svc1 = M2mApiService::new(seq1.clone(), prov1.clone(), comp1.clone());
    let p2m1 = P2mApiService::new(seq1, prov1, comp1, m2m.clone()).with_enrolled_resource(
        1,
        3,
        Resource::new_process_mock(1),
        stream1,
    );

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        m2m.register_middleware(node1, m2m_svc1).await;
        for ip in &remote_ips {
            let seq = WaitingQueueService::new(SequencerService::default(), None);
            let prov = ProvenanceService::new(ip.clone());
            let consent = ConsentService::default();
            let comp = ComplianceService::new(ip.clone(), consent);
            m2m.register_middleware(ip.clone(), M2mApiService::new(seq, prov, comp)).await;
        }
    });

    (p2m1, m2m)
}

fn distributed_io(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("distributed_io");

    for n in NODE_COUNTS {
        group.bench_with_input(BenchmarkId::from_parameter(format!("{}_nodes", n)), &n, |b, &n| {
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

fn grpc_local(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let addr: std::net::SocketAddr = "127.0.0.1:18753".parse().unwrap();
    let node_id = "127.0.0.1".to_string();
    let file = Resource::new_file("/tmp/bench".to_string());

    let seq = WaitingQueueService::new(SequencerService::default(), None);
    let prov = ProvenanceService::new(node_id.clone());
    let comp = ComplianceService::new(node_id.clone(), ConsentService::default());

    prov.set_references(file.clone(), build_refs(&node_id, GRPC_REF_COUNT));

    let p2m_service = P2mApiService::new(seq, prov, comp, M2mNop).with_enrolled_resource(
        1,
        3,
        Resource::new_process_mock(1),
        file,
    );

    rt.spawn(async move {
        Server::builder()
            .add_service(P2mServer::new(P2mHandler::new(p2m_service)))
            .serve(addr)
            .await
            .unwrap();
    });

    // Give the server a moment to start
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Connect client
    let mut client: proto::p2m_client::P2mClient<_> =
        rt.block_on(proto::p2m_client::P2mClient::connect(format!("http://{addr}"))).unwrap();

    let mut group = c.benchmark_group("grpc_local");
    group.bench_function("io", |b| {
        b.iter(|| {
            rt.block_on(async {
                let grant: proto::messages::Grant = client
                    .p2m_io_request(tonic::Request::new(proto::messages::IoInfo {
                        process_id: 1,
                        file_descriptor: 3,
                        flow: proto::primitives::Flow::Input as i32,
                    }))
                    .await
                    .unwrap()
                    .into_inner();

                let _: proto::messages::Ack = client
                    .p2m_io_report(tonic::Request::new(proto::messages::IoResult {
                        process_id: 1,
                        file_descriptor: 3,
                        grant_id: grant.id,
                        result: true,
                    }))
                    .await
                    .unwrap()
                    .into_inner();
            })
        });
    });
    group.finish();
}

fn grpc_distributed(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Node2 (127.0.0.2): serves M2M gRPC, handles inbound CheckSourceCompliance
    let node2_socket: std::net::SocketAddr = "127.0.0.2:50051".parse().unwrap();

    // Node1 (127.0.0.1): receives P2M I/O, triggers M2M call to node2
    let node1_socket: std::net::SocketAddr = "127.0.0.1:50051".parse().unwrap();
    let file = Resource::new_file("/tmp/bench".to_string());
    let process_mock = Resource::new_process_mock(1);

    let seq = WaitingQueueService::new(SequencerService::default(), None);
    let prov = ProvenanceService::new(node1_socket.ip().to_string());
    let comp = ComplianceService::new(node1_socket.ip().to_string(), ConsentService::default());

    // File's provenance from node2
    prov.set_references(file.clone(), build_refs(&node2_socket.ip().to_string(), GRPC_REF_COUNT));

    let m2m = M2mApiService::new(seq.clone(), prov.clone(), comp.clone());
    let p2m1 = P2mApiService::new(seq, prov, comp, M2mGrpc::mock()).with_enrolled_resource(
        1,
        3,
        process_mock,
        file,
    );

    rt.spawn(async move {
        Server::builder()
            .add_service(M2mServer::new(M2mHandler::new(m2m)))
            .add_service(P2mServer::new(P2mHandler::new(p2m1)))
            .serve(node1_socket)
            .await
            .unwrap();
    });

    std::thread::sleep(std::time::Duration::from_millis(10));

    // Connect P2M client to node1
    let mut client: proto::p2m_client::P2mClient<_> = rt
        .block_on(proto::p2m_client::P2mClient::connect(format!("http://{node1_socket}")))
        .unwrap();

    let mut group = c.benchmark_group("grpc_distributed");
    group.bench_function("io", |b| {
        b.iter(|| {
            rt.block_on(async {
                let grant: proto::messages::Grant = client
                    .p2m_io_request(tonic::Request::new(proto::messages::IoInfo {
                        process_id: 1,
                        file_descriptor: 3,
                        flow: proto::primitives::Flow::Input as i32,
                    }))
                    .await
                    .unwrap()
                    .into_inner();

                let _: proto::messages::Ack = client
                    .p2m_io_report(tonic::Request::new(proto::messages::IoResult {
                        process_id: 1,
                        file_descriptor: 3,
                        grant_id: grant.id,
                        result: true,
                    }))
                    .await
                    .unwrap()
                    .into_inner();
            })
        });
    });
    group.finish();
}

criterion_group!(benches, local_io, distributed_io, grpc_local, grpc_distributed);
criterion_main!(benches);
