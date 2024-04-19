use tonic::{transport::Server, Request, Response, Status};
use trace2e::{Ct, Io, Ack, trace2e_server::{Trace2e, Trace2eServer}};

pub mod trace2e {
  tonic::include_proto!("trace2e");
}

#[derive(Debug, Default)]
pub struct Trace2eService {}

#[tonic::async_trait]
impl Trace2e for Trace2eService {
    async fn ct_event(&self, request: Request<Ct>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();
        println!("CT Event: FD {} | IN {} | OUT {} | REMOTE {} | {} @ {}", r.file_descriptor, r.input, r.output, r.remote, r.container, r.resource_identifier);
        Ok(Response::new(trace2e::Ack { authorization: { true }}))
    }
    async fn io_event(&self, request: Request<Io>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();
        println!("IO Event: FD {} | {} on {}", r.file_descriptor, r.method, r.container);
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