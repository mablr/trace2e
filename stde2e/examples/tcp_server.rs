use std::{io::Write, net::TcpListener};

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8888").unwrap();
    match listener.accept() {
        Ok((mut s, _)) => {
            s.write(b"Hello, world!\n").unwrap();
            Ok(())
        }
        Err(e) => Err(e),
    }
}
