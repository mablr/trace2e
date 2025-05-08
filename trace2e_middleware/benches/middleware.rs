use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tokio::runtime::Runtime;
use trace2e_middleware::{identifier::Identifier, p2m_service::{p2m::{p2m_server::P2m, Flow, IoInfo, IoResult, LocalCt}, P2mService}, traceability::{spawn_traceability_server, TraceabilityClient}};

fn bench_traceability_server(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let traceability = rt.block_on(async {
        TraceabilityClient::new(spawn_traceability_server())
    });

    let process_id = Identifier::new_process(1, 1, String::new());
    let file_id = Identifier::new_file("/dev/null".to_string());

    rt.block_on(async {
        traceability.register_container(process_id.clone()).await.unwrap();
        traceability.register_container(file_id.clone()).await.unwrap();
    });

    c.bench_function("traceability_server", |b| {
        b.iter(|| black_box(rt.block_on(async {
            let grant_id = traceability.declare_flow(process_id.clone(), file_id.clone(), true).await.unwrap();
            traceability.record_flow(grant_id).await.unwrap();
        })));
    });
}

fn bench_grpc_service(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let traceability = rt.block_on(async {
        TraceabilityClient::new(spawn_traceability_server())
    });
    let client = P2mService::new(traceability);

    rt.block_on(async {
        client.local_enroll(tonic::Request::new(LocalCt {
            process_id: 1,
            file_descriptor: 3,
            path: "/dev/null".to_string(),
        })).await.unwrap();
    });

    c.bench_function("grpc_service", |b| {
        b.iter(|| black_box(rt.block_on(async {
            let grant_id = client.io_request(tonic::Request::new(IoInfo {
                process_id: 1,
                file_descriptor: 3,
                flow: Flow::Output.into(),
            })).await.unwrap().into_inner().id;
            client.io_report(tonic::Request::new(IoResult {
                process_id: 1,
                file_descriptor: 3,
                grant_id,
                result: true,
            })).await.unwrap().into_inner()
        })));
    });
}

criterion_group!(
    benches,
    bench_traceability_server,
    bench_grpc_service,
);
criterion_main!(benches);