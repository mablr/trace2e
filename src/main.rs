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
    containers_states: Arc<RwLock<HashMap<String, bool>>>,
    identifiers_map: Arc<RwLock<HashMap<(u32, i32), String>>>,
    queuing_handler: Arc<Mutex<HashMap<String, VecDeque<oneshot::Sender<()>>>>>,
}

impl Trace2eService {
    pub fn new() -> Self {
        Trace2eService { 
            containers_states: Arc::new(RwLock::new(HashMap::new())),
            identifiers_map: Arc::new(RwLock::new(HashMap::new())),
            queuing_handler: Arc::new(Mutex::new(HashMap::new()))
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

        if {
            let containers_states = self.containers_states.read().await;
            containers_states.get(&r.resource_identifier).is_some()
        } == false {
            let mut containers_states = self.containers_states.write().await;
            containers_states.insert(r.resource_identifier.clone(), true);
        }
        if {
            let identifiers_map = self.identifiers_map.read().await;
            identifiers_map.get(&(r.process_id.clone(), r.file_descriptor.clone())).is_some()
        } == false {
            let mut identifiers_map = self.identifiers_map.write().await;
            identifiers_map.insert((r.process_id.clone(), r.file_descriptor.clone()), r.resource_identifier.clone());
        }
        Ok(Response::new(trace2e::Ack {}))
    }

    async fn io_event(&self, request: Request<Io>) -> Result<Response<Grant>, Status> {
        let r = request.into_inner();

        println!("PID: {} | IO Event Requested: FD {} | {} on {}", r.process_id, r.file_descriptor, r.method, r.container);

        if let Some(resource_identifier) = {
            let identifiers_map = self.identifiers_map.read().await;
            identifiers_map.get(&(r.process_id.clone(), r.file_descriptor.clone())).cloned()
        } {
            if let Some(available) = {
                let containers_states = self.containers_states.read().await;
                containers_states.get(&resource_identifier).cloned()
            } {
                // CT is already the track list
                if available {
                    // CT is available
                    let mut containers_states = self.containers_states.write().await;
                    containers_states.insert(resource_identifier.clone(), false);
                } else {
                    // CT is reserved
                    // Set up a oneshot channel to be notified when it becomes available
                    let (tx, rx) = oneshot::channel();

                    // Queuing the channel
                    {
                        let mut queuing_handler = self.queuing_handler.lock().await;
                        if let Some(queue) = queuing_handler.get_mut(&resource_identifier) {
                            queue.push_back(tx);
                        } else {
                            let mut queue = VecDeque::new();
                            queue.push_back(tx);
                            queuing_handler.insert(resource_identifier.clone(), queue);
                        }
                    }

                    // Wait until the CT becomes available
                    println!("-> Wait a bit PID {}", r.process_id);
                    rx.await.unwrap();

                    // CT is now available, set it as reserved
                    let mut containers_states = self.containers_states.write().await;
                    containers_states.insert(resource_identifier.clone(), false);
                    println!("<- Now it's your turn PID {}", r.process_id);
                }
            } else {
                println!("Ressource {} does not exist", resource_identifier);
            }
        } else {
            println!("CT {} is not tracked", r.file_descriptor);
        }
        println!("PID: {} | IO Event Authorized: FD {} | {} on {}", r.process_id, r.file_descriptor, r.method, r.container);

        Ok(Response::new(trace2e::Grant { file_descriptor: r.file_descriptor }))
    }

    async fn done_io_event(&self, request: Request<Io>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        if let Some(resource_identifier) = {
            let identifiers_map = self.identifiers_map.read().await;
            identifiers_map.get(&(r.process_id.clone(), r.file_descriptor.clone())).cloned()
        } {
            if let Some(available) = {
                let containers_states = self.containers_states.read().await;
                containers_states.get(&resource_identifier).cloned()
            } {
                if !available {
                    // File was reserved, set it as available now
                    let mut containers_states = self.containers_states.write().await;
                    containers_states.insert(resource_identifier.clone(), true);

                    // Notify that the file is available
                    {
                        let mut queuing_handler = self.queuing_handler.lock().await;
                        if let Some(queue) = queuing_handler.get_mut(&resource_identifier) {
                            if let Some(channel) = queue.pop_front() {
                                channel.send(()).unwrap();
                            }
                        }
                    }
                } else {
                    println!("Ressource {} is already available", resource_identifier);
                }
            } else {
                println!("Ressource {} does not exist", resource_identifier);
            }
        } else {
            println!("CT {} is not tracked", r.file_descriptor);
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
