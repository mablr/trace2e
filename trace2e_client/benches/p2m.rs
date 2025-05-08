use criterion::{black_box, criterion_group, criterion_main, Criterion};

use trace2e_client::{local_enroll, io_request, io_report};

fn bench_p2m_service(c: &mut Criterion) {
    local_enroll("/dev/null", 3);

    #[cfg(feature = "dbus")]
    let bench_id = "p2m-dbus";
    #[cfg(not(feature = "dbus"))]
    let bench_id = "p2m-grpc";

    c.bench_function(bench_id, |b| {
        b.iter(|| black_box( {
            let grant_id = io_request(3, 1).unwrap();
            io_report(3, grant_id, true).unwrap();
        }));
    });
}

criterion_group!(
    benches,
    bench_p2m_service,
);
criterion_main!(benches);