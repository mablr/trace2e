use stde2e::{io::Read, net::TcpStream};
use std::time::Instant;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let start_connect_time = Instant::now();
    let mut stream = TcpStream::connect("127.0.0.1:8888").unwrap();
    let connect_time = start_connect_time.elapsed();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let mut buf = [0; 16];
    let start_read_time = Instant::now();
    stream.read(&mut buf).unwrap();
    let read_time = start_read_time.elapsed();
    println!("\"stde2e\": \"{:?}\"", connect_time + read_time);
    Ok(())
}
