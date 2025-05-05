use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::fs::File;
use stde2e::fs::File as FileE2E;

fn bench_read_e2e(c: &mut Criterion) {
    let mut small_file = File::create("read_small.txt").unwrap();
    std::io::Write::write_all(&mut small_file, &vec![1u8; 64]).unwrap();
    let mut large_file = File::create("read_large.txt").unwrap();
    std::io::Write::write_all(&mut large_file, &vec![1u8; 262144]).unwrap();

    let mut buf = vec![0u8; 262144];
    let mut e2e_small_file = FileE2E::open("read_small.txt").unwrap();
    let mut e2e_large_file: File = FileE2E::open("read_large.txt").unwrap();

    c.bench_function("read_e2e_small_buffer", |b| {
        b.iter(|| {
            black_box(stde2e::io::Read::read_to_end(&mut e2e_small_file, &mut buf).unwrap());
        })
    });

    c.bench_function("read_e2e_large_buffer", |b| {
        b.iter(|| {
            black_box(stde2e::io::Read::read_to_end(&mut e2e_large_file, &mut buf).unwrap());
        })
    });
}

fn bench_write_e2e(c: &mut Criterion) {
    let small_buf = vec![1u8; 64];
    let large_buf = vec![1u8; 262144];
    let mut e2e_small_file = FileE2E::create("write_small.txt").unwrap();
    let mut e2e_large_file = FileE2E::create("write_large.txt").unwrap();

    c.bench_function("write_e2e_small_buffer", |b| {
        b.iter(|| {
            black_box(stde2e::io::Write::write_all(&mut e2e_small_file, &small_buf).unwrap());
        })
    });

    c.bench_function("write_e2e_large_buffer", |b| {
        b.iter(|| {
            black_box(stde2e::io::Write::write_all(&mut e2e_large_file, &large_buf).unwrap());
        })
    });
}

fn bench_read_std(c: &mut Criterion) {
    let mut small_file = File::create("read_small.txt").unwrap();
    std::io::Write::write_all(&mut small_file, &vec![1u8; 64]).unwrap();
    let mut large_file = File::create("read_large.txt").unwrap();
    std::io::Write::write_all(&mut large_file, &vec![1u8; 262144]).unwrap();

    let mut buf = vec![0u8; 262144];
    let mut small_file = File::open("read_small.txt").unwrap();
    let mut large_file = File::open("read_large.txt").unwrap();

    c.bench_function("read_std_small_buffer", |b| {
        b.iter(|| {
            black_box(std::io::Read::read_to_end(&mut small_file, &mut buf).unwrap());
        })
    });

    c.bench_function("read_std_large_buffer", |b| {
        b.iter(|| {
            black_box(std::io::Read::read_to_end(&mut large_file, &mut buf).unwrap());
        })
    });
}


fn bench_write_std(c: &mut Criterion) {
    let small_buf = vec![1u8; 64];
    let large_buf = vec![1u8; 262144];
    let mut small_file = File::create("write_small.txt").unwrap();
    let mut large_file = File::create("write_large.txt").unwrap();

    c.bench_function("write_std_small_buffer", |b| {
        b.iter(|| {
            black_box(std::io::Write::write_all(&mut small_file, &small_buf).unwrap());
        })
    });

    c.bench_function("write_std_large_buffer", |b| {
        b.iter(|| {
            black_box(std::io::Write::write_all(&mut large_file, &large_buf).unwrap());
        })
    });
}

criterion_group!(
    benches,
    bench_read_e2e,
    bench_write_e2e,
    bench_read_std,
    bench_write_std
);
criterion_main!(benches);
