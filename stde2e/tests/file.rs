use std::time::Instant;

use stde2e::{fs::File, io::Write};

#[test]
#[ignore]
fn stde2e_file_create() {
    let time = Instant::now();
    let mut f = File::create("test1.txt").unwrap();
    f.write(b"test").unwrap();

    println!("Elapsed time {:?}", time.elapsed());
}
