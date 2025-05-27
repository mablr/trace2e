use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_file_read_e2e(c: &mut Criterion) {
    let mut small_file = std::fs::File::create("read_small.txt").unwrap();
    std::io::Write::write_all(&mut small_file, &vec![1u8; 64]).unwrap();
    let mut large_file = std::fs::File::create("read_large.txt").unwrap();
    std::io::Write::write_all(&mut large_file, &vec![1u8; 262144]).unwrap();

    let mut buf = vec![0u8; 262144];
    let mut e2e_small_file = stde2e::fs::File::open("read_small.txt").unwrap();
    let mut e2e_large_file = stde2e::fs::File::open("read_large.txt").unwrap();

    c.bench_function("file_read_e2e_small", |b| {
        b.iter(|| {
            black_box(stde2e::io::Read::read_to_end(&mut e2e_small_file, &mut buf).unwrap());
        })
    });

    c.bench_function("file_read_e2e_large", |b| {
        b.iter(|| {
            black_box(stde2e::io::Read::read_to_end(&mut e2e_large_file, &mut buf).unwrap());
        })
    });
}

fn bench_file_write_e2e(c: &mut Criterion) {
    let small_buf = vec![1u8; 64];
    let large_buf = vec![1u8; 262144];
    let mut e2e_small_file = stde2e::fs::File::create("write_small.txt").unwrap();
    let mut e2e_large_file = stde2e::fs::File::create("write_large.txt").unwrap();

    c.bench_function("file_write_e2e_small", |b| {
        b.iter(|| {
            black_box(stde2e::io::Write::write_all(&mut e2e_small_file, &small_buf).unwrap());
        })
    });

    c.bench_function("file_write_e2e_large", |b| {
        b.iter(|| {
            black_box(stde2e::io::Write::write_all(&mut e2e_large_file, &large_buf).unwrap());
        })
    });
}

fn bench_file_read_std(c: &mut Criterion) {
    let mut small_file = std::fs::File::create("read_small.txt").unwrap();
    std::io::Write::write_all(&mut small_file, &vec![1u8; 64]).unwrap();
    let mut large_file = std::fs::File::create("read_large.txt").unwrap();
    std::io::Write::write_all(&mut large_file, &vec![1u8; 262144]).unwrap();

    let mut buf = vec![0u8; 262144];
    let mut small_file = std::fs::File::open("read_small.txt").unwrap();
    let mut large_file = std::fs::File::open("read_large.txt").unwrap();

    c.bench_function("file_read_std_small", |b| {
        b.iter(|| {
            black_box(std::io::Read::read_to_end(&mut small_file, &mut buf).unwrap());
        })
    });

    c.bench_function("file_read_std_large", |b| {
        b.iter(|| {
            black_box(std::io::Read::read_to_end(&mut large_file, &mut buf).unwrap());
        })
    });
}

fn bench_file_write_std(c: &mut Criterion) {
    let small_buf = vec![1u8; 64];
    let large_buf = vec![1u8; 262144];
    let mut small_file = std::fs::File::create("write_small.txt").unwrap();
    let mut large_file = std::fs::File::create("write_large.txt").unwrap();

    c.bench_function("file_write_std_small", |b| {
        b.iter(|| {
            black_box(std::io::Write::write_all(&mut small_file, &small_buf).unwrap());
        })
    });

    c.bench_function("file_write_std_large", |b| {
        b.iter(|| {
            black_box(std::io::Write::write_all(&mut large_file, &large_buf).unwrap());
        })
    });
}

fn bench_tcp_stream_write_e2e(c: &mut Criterion) {
    let buf = vec![1u8; 262144];

    // Start server in a separate thread
    let server = stde2e::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();

    let server_thread = std::thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        loop {
            let mut read_buf = vec![0u8; 262144];
            match std::io::Read::read(&mut stream, &mut read_buf) {
                Ok(0) => break, // Connection closed
                Ok(_) => continue,
                Err(_) => break,
            }
        }
    });

    let mut client = stde2e::net::TcpStream::connect(addr).unwrap();

    c.bench_function("tcp_stream_write_e2e", |b| {
        b.iter(|| {
            black_box(stde2e::io::Write::write_all(&mut client, &buf).unwrap());
        })
    });

    drop(client); // Close the connection
    server_thread.join().unwrap();
}

fn bench_tcp_stream_write_std(c: &mut Criterion) {
    let buf = vec![1u8; 262144];

    // Start server in a separate thread
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();

    let server_thread = std::thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        loop {
            let mut read_buf = vec![0u8; 262144];
            match std::io::Read::read(&mut stream, &mut read_buf) {
                Ok(0) => break, // Connection closed
                Ok(_) => continue,
                Err(_) => break,
            }
        }
    });

    let mut client = std::net::TcpStream::connect(addr).unwrap();

    c.bench_function("tcp_stream_write_std", |b| {
        b.iter(|| {
            black_box(std::io::Write::write_all(&mut client, &buf).unwrap());
        })
    });

    drop(client); // Close the connection
    server_thread.join().unwrap();
}

criterion_group!(
    benches,
    bench_file_read_e2e,
    bench_file_write_e2e,
    bench_file_read_std,
    bench_file_write_std,
    bench_tcp_stream_write_e2e,
    bench_tcp_stream_write_std
);
criterion_main!(benches);
