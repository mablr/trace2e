use std::io::Read;

use stde2e::{
    io::Write,
    net::{TcpListener, TcpStream},
};

#[test]
fn stde2e_stream_server() {
    let l = TcpListener::bind("127.0.0.1:8081").unwrap();
    let (mut s, _) = l.accept().unwrap();
    s.write("test".as_bytes()).unwrap();
}

#[test]
fn stde2e_stream_client() {
    std::thread::sleep(std::time::Duration::from_millis(10)); // let spawn the tcp listener
    let mut s = TcpStream::connect("127.0.0.1:8081").unwrap();
    let mut buf = String::new();
    assert_eq!(s.read_to_string(&mut buf).unwrap(), 4);
    assert_eq!(buf, "test")
}
