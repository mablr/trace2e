use tonic::{transport::Server, Request, Response, Status};
use trace2e::{Ct, Io, Grant, Ack, trace2e_server::{Trace2e, Trace2eServer}};
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot, RwLock};
use std::collections::{HashMap, VecDeque};

pub mod trace2e {
  tonic::include_proto!("trace2e");
}

#[derive(Debug)]
pub struct Trace2eService {
    containers: Arc<RwLock<HashMap<i32, bool>>>,
    containers_handler: Arc<Mutex<HashMap<i32, VecDeque<oneshot::Sender<()>>>>>,
}

impl Trace2eService {
    pub fn new() -> Self {
        Trace2eService { 
            containers: Arc::new(RwLock::new(HashMap::new())),
            containers_handler: Arc::new(Mutex::new(HashMap::new()))
        }
    }
}

#[tonic::async_trait]
impl Trace2e for Trace2eService {
    async fn ct_event(&self, request: Request<Ct>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();
        println!("PID: {} | CT Event Notified: FD {} | IN {} | OUT {} | REMOTE {} | {} @ {}", r.process_id,  r.file_descriptor, r.input, r.output, r.remote, r.container, r.resource_identifier); 

        // Adding CT to the tracklist (and set it as available -> have to be discussed...)
        // could be more fine-grained using IN OUT flags
        let mut containers = self.containers.write().await;
        if containers.get(&r.file_descriptor).is_some() == false {
            containers.insert(r.file_descriptor.clone(), true);
        }

        Ok(Response::new(trace2e::Ack {}))
    }

    async fn io_event(&self, request: Request<Io>) -> Result<Response<Grant>, Status> {
        let r = request.into_inner();

        println!("PID: {} | IO Event Requested: FD {} | {} on {}", r.process_id, r.file_descriptor, r.method, r.container);

        let containers = self.containers.read().await;
        let mut containers_handler = self.containers_handler.lock().await;

        if let Some(available) = containers.get(&r.file_descriptor) {
            // CT is already the track list
            if *available {
                // CT is available
                drop(containers); // Switching to write mode

                let mut containers = self.containers.write().await;
                containers.insert(r.file_descriptor.clone(), false);
            } else {
                // CT is reserved
                // Set up a oneshot channel to be notified when it becomes available
                let (tx, rx) = oneshot::channel();

                // Queuing the channel
                if let Some(queue) = containers_handler.get_mut(&r.file_descriptor) {
                    queue.push_back(tx);
                } else {
                    let mut queue = VecDeque::new();
                    queue.push_back(tx);
                    containers_handler.insert(r.file_descriptor.clone(), queue);
                }
                
                // Release the locks before waiting to avoid deadlock
                drop(containers);
                drop(containers_handler);

                // Wait until the CT becomes available
                println!("-> Wait a bit PID {}", r.process_id);
                rx.await.unwrap();

                // CT is now available, set it as reserved
                let mut containers = self.containers.write().await;
                containers.insert(r.file_descriptor.clone(), false);
                println!("<- Now it's your turn PID {}", r.process_id);
            }
        } else {
            // File not found
            // Err(Status::not_found(format!("File {} is not tracked", r.file_descriptor)))
            println!("File {} is not tracked", r.file_descriptor);
        }

        println!("PID: {} | IO Event Authorized: FD {} | {} on {}", r.process_id, r.file_descriptor, r.method, r.container);

        Ok(Response::new(trace2e::Grant { file_descriptor: r.file_descriptor }))
    }

    async fn done_io_event(&self, request: Request<Io>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        let mut containers = self.containers.write().await;
        let mut containers_handler = self.containers_handler.lock().await;

        if let Some(available) = containers.get(&r.file_descriptor) {
            if !*available {
                // File was reserved, set it as available now
                containers.insert(r.file_descriptor.clone(), true);

                // Notify that the file is available
                if let Some(queue) = containers_handler.get_mut(&r.file_descriptor) {
                    if let Some(channel) = queue.pop_front() {
                        channel.send(()).unwrap();
                    }
                }
            } else {
                // File was already available
                // Err(Status::invalid_argument(format!("File {} is already available", r.file_descriptor)))
                println!("File {} is already available", r.file_descriptor);
            }
        } else {
            // File not found
            // Err(Status::not_found(format!("File {} is not tracked", r.file_descriptor)))
            println!("File {} is not tracked", r.file_descriptor);
        }

        println!("PID: {} | IO Event Done: FD {} | {} on {}", r.process_id, r.file_descriptor, r.method, r.container);     

        Ok(Response::new(trace2e::Ack{}))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = "[::1]:8080".parse().unwrap();
    let trace2e_service = Trace2eService::new();

    Server::builder().add_service(Trace2eServer::new(trace2e_service))
        .serve(address)
        .await?;

    Ok(())       
}