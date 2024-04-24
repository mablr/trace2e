use tonic::{transport::Server, Request, Response, Status};
use trace2e::{Ct, Io, Ack, trace2e_server::{Trace2e, Trace2eServer}};
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod trace2e {
  tonic::include_proto!("trace2e");
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Counter {
    value: u64,
}

impl Counter {
    pub fn new() -> Self {
        Counter {
            value: 0,
        }
    }
    pub fn get(self) -> u64 {
        self.value
    }
    pub fn increment(&mut self) {
        self.value += 1;
    }
}


#[derive(Debug, Default)]
pub struct Trace2eService {
    ri_counter: Arc<Mutex<Counter>>,
}

#[tonic::async_trait]
impl Trace2e for Trace2eService {
    async fn ct_event(&self, request: Request<Ct>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();
        println!("CT Event: FD {} | IN {} | OUT {} | REMOTE {} | {} @ {}", r.file_descriptor, r.input, r.output, r.remote, r.container, r.resource_identifier);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let mut ri_counter = self.ri_counter.lock().await;
        ri_counter.increment();
        Ok(Response::new(trace2e::Ack { authorization: { true }}))
    }
    async fn io_event(&self, request: Request<Io>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();
        let ri_counter = self.ri_counter.lock().await;
        println!("{}", ri_counter.get());
        println!("IO Event: FD {} | {} on {}", r.file_descriptor, r.method, r.container);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        Ok(Response::new(trace2e::Ack { authorization: { true }}))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = "[::1]:8080".parse().unwrap();
    let trace2e_service = Trace2eService::default();

    Server::builder().add_service(Trace2eServer::new(trace2e_service))
        .serve(address)
        .await?;
    Ok(())       
}