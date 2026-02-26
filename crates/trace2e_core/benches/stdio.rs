use std::fs::File;
use std::io::{Read, Write};

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

const SIZES: [usize; 3] = [100, 10_000, 1_000_000];

fn make_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

fn bench_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("std_write");

    for &size in &SIZES {
        let data = make_data(size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            b.iter(|| {
                let mut f = File::create("/tmp/bench_stdio_write").unwrap();
                f.write_all(data).unwrap();
                f.flush().unwrap();
            });
        });
    }

    group.finish();
}

fn bench_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("std_read");

    for &size in &SIZES {
        // Prepare file
        let path = format!("/tmp/bench_stdio_read_{size}");
        let data = make_data(size);
        {
            let mut f = File::create(&path).unwrap();
            f.write_all(&data).unwrap();
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut buf = vec![0u8; size];
            b.iter(|| {
                let mut f = File::open(&path).unwrap();
                f.read_exact(&mut buf).unwrap();
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_write, bench_read);
criterion_main!(benches);
