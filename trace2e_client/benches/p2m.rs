use criterion::{black_box, criterion_group, criterion_main, Criterion};

use trace2e_client::{io_report, io_request, local_enroll, Flow};

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

fn bench_p2m_service_control(c: &mut Criterion) {
    use std::os::fd::AsRawFd;
    let small_buf = vec![1u8; 64];
    let mut file = std::fs::File::create("test.txt").unwrap();
    local_enroll("test.txt", file.as_raw_fd());

    #[cfg(feature = "dbus")]
    let bench_id = "p2m-dbus-control";
    #[cfg(not(feature = "dbus"))]
    let bench_id = "p2m-grpc-control";

    c.bench_function(bench_id, |b| {
        b.iter(|| black_box( {
            if let Ok(grant_id) = io_request(file.as_raw_fd(), Flow::Output.into()) {
                let result = std::io::Write::write(&mut file, &small_buf);
                io_report(file.as_raw_fd(), grant_id, result.is_ok())?;
                result
            } else {
                Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
            }
        }));
    });
}

criterion_group!(
    benches,
    bench_p2m_service,
    bench_p2m_service_control,
);
criterion_main!(benches);