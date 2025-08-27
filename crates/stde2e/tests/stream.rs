use stde2e::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};
use tokio::runtime::Handle;

#[tokio::test]
#[ignore]
async fn stde2e_net_stream_instantiation() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8081")?;
    Handle::current().spawn(async move {
        let (mut server, _) = listener.accept().unwrap();
        server.write("test".as_bytes()).unwrap();
    });

    // Give the server a brief moment to start
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut client = TcpStream::connect("127.0.0.1:8081")?;

    // Give the server a brief moment to write
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut buf = String::new();
    assert_eq!(client.read_to_string(&mut buf).unwrap(), 4);
    assert_eq!(buf, "test");
    Ok(())
}
