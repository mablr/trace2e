use stde2e::{io::Read, net::TcpStream};
use std::time::Instant;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let start_time = Instant::now();
    let mut stream = TcpStream::connect("127.0.0.1:8888").unwrap();
    let mut buf = [0; 16];
    stream.read(&mut buf).unwrap();
    let end_time = start_time.elapsed();
    println!("\"stde2e\": \"{:?}\",", end_time);
    Ok(())
}
