use crate::middleware::{LocalCt, GRPC_CLIENT, TOKIO_RUNTIME};
use std::fs::canonicalize;
use std::fs::File as StdFile;
use std::fs::OpenOptions as StdOpenOptions;
use std::os::fd::AsRawFd;
use std::process::id;

fn middleware_request(path: String, fd: i32) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = GRPC_CLIENT.clone();

    let request = tonic::Request::new(LocalCt {
        process_id: id(),
        file_descriptor: fd,
        path: canonicalize(path).unwrap().display().to_string(),
    });
    let _ = TOKIO_RUNTIME.block_on(client.local_enroll(request));
    Ok(())
}

pub struct File;

impl File {
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> std::io::Result<StdFile> {
        let path_ref = path.as_ref();
        let file = StdFile::open(path_ref)?;
        let _ = middleware_request(path_ref.display().to_string(), file.as_raw_fd());
        Ok(file)
    }
    pub fn create<P: AsRef<std::path::Path>>(path: P) -> std::io::Result<StdFile> {
        let path_ref = path.as_ref();
        let file = StdFile::create(path_ref)?;
        let _ = middleware_request(path_ref.display().to_string(), file.as_raw_fd());
        Ok(file)
    }
    pub fn create_new<P: AsRef<std::path::Path>>(path: P) -> std::io::Result<StdFile> {
        let path_ref = path.as_ref();
        let file = StdFile::create_new(path_ref)?;
        let _ = middleware_request(path_ref.display().to_string(), file.as_raw_fd());
        Ok(file)
    }
    pub fn options() -> OpenOptions {
        OpenOptions::new()
    }
}

pub struct OpenOptions {
    options: StdOpenOptions,
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
}

impl OpenOptions {
    pub fn new() -> Self {
        OpenOptions {
            options: StdOpenOptions::new(),
            read: false,
            write: false,
            append: false,
            truncate: false,
            create: false,
            create_new: false,
        }
    }
    pub fn read(&mut self, read: bool) -> &mut Self {
        self.options.read(read);
        self.read = read;
        self
    }
    pub fn write(&mut self, write: bool) -> &mut Self {
        self.options.write(write);
        self.write = write;
        self
    }
    pub fn append(&mut self, append: bool) -> &mut Self {
        self.options.append(append);
        self.append = append;
        self
    }
    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.options.truncate(truncate);
        self.truncate = truncate;
        self
    }
    pub fn create(&mut self, create: bool) -> &mut Self {
        self.options.create(create);
        self.create = create;
        self
    }
    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.options.create_new(create_new);
        self.create_new = create_new;
        self
    }
    pub fn open<P: AsRef<std::path::Path>>(&self, path: P) -> std::io::Result<std::fs::File> {
        let path_ref = path.as_ref();
        let file = self.options.open(path_ref)?;
        let _ = middleware_request(path_ref.display().to_string(), file.as_raw_fd());
        Ok(file)
    }
}
