use p2m::{Ct, Io, Grant, Ack, p2m_server::P2m};
use tonic::{Request, Response, Status};
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot, RwLock};
use std::collections::{HashMap, VecDeque};

pub mod p2m {
    tonic::include_proto!("trace2e_api");
    pub const FILE_DESCRIPTOR_SET: &[u8] = include_bytes!("../target/trace2e_api_descriptor.bin");
}

#[derive(Debug)]
pub struct P2mService {
    containers_states: Arc<Mutex<HashMap<String, bool>>>,
    identifiers_map: Arc<RwLock<HashMap<(u32, i32), String>>>,
    queuing_handler: Arc<Mutex<HashMap<String, VecDeque<oneshot::Sender<()>>>>>,
}

impl P2mService {
    pub fn new() -> Self {
        P2mService { 
            containers_states: Arc::new(Mutex::new(HashMap::new())),
            identifiers_map: Arc::new(RwLock::new(HashMap::new())),
            queuing_handler: Arc::new(Mutex::new(HashMap::new()))
        }
    }
}

#[tonic::async_trait]
impl P2m for P2mService {
    async fn ct_event(&self, request: Request<Ct>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        #[cfg(feature = "verbose")]
        println!("PID: {} | CT Event Notified: FD {} | IN {} | OUT {} | REMOTE {} | {} @ {}", r.process_id,  r.file_descriptor, r.input, r.output, r.remote, r.container, r.resource_identifier); 

        /*
        Adding CT to the tracklist

        If it is already in the hashmap, do nothing to avoid erasing of the previous container state.
        */
        {
            let mut containers_states = self.containers_states.lock().await;
            if containers_states.get(&r.resource_identifier).is_some() == false {
                containers_states.insert(r.resource_identifier.clone(), true);
            }
        }
        /* 
        At the process level, there is a bijection between the file_descriptor and the designated underlying container.

        2 cases can be outlined:
        - No existing entry for tuple (process_id, file_descriptor) 
          It means a process declared a new fd mapped to a container.
            => The relation must be added to the hashmap. 
        - There is an entry
          It means that a process was previously declared as fd mapped to a container.
          Since the procedure was called with the existing (process_id, file_descriptor),
          it also means that the previous fd was destroyed and a new one declared,
          so that the new one could be mapped to a different container.
            => The relation must be inserted into the hashmap.

        So in any case we have to insert the relation (process_id, file_descriptor) - resource_identifier.
         */
        {
            let mut identifiers_map = self.identifiers_map.write().await;
            identifiers_map.insert((r.process_id.clone(), r.file_descriptor.clone()), r.resource_identifier.clone());
        }
        Ok(Response::new(Ack {}))
    }

    async fn io_event(&self, request: Request<Io>) -> Result<Response<Grant>, Status> {
        let r = request.into_inner();

        #[cfg(feature = "verbose")]
        println!("PID: {} | IO Event Requested: FD {} | {} on {}", r.process_id, r.file_descriptor, r.method, r.container);

        if let Some(resource_identifier) = {
            let identifiers_map = self.identifiers_map.read().await;
            identifiers_map.get(&(r.process_id.clone(), r.file_descriptor.clone())).cloned()
        } {
            // Obtain MutexGuard on self.containers_states only if an entry is associated with the resource_identifier 
            // and the state of this entry is true.
            // TODO: put it into a function
            if let Some(mut containers_states) = {
                let containers_states = self.containers_states.lock().await;
                if containers_states.get(&resource_identifier) == Some(&true) {
                    Some(containers_states)
                } else { // If the state is false or the entry doesn't exist, return None.
                    None
                }
            } {
                // CT is in the track list and is available, so it can be reserved
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
                #[cfg(feature = "verbose")]
                println!("-> Wait a bit PID {}", r.process_id);
                
                rx.await.unwrap();

                // CT is now reserved for the current process
                #[cfg(feature = "verbose")]
                println!("<- Now it's your turn PID {}", r.process_id);
            }
        } else {
            #[cfg(feature = "verbose")]
            println!("CT {} is not tracked", r.file_descriptor);
        }
        
        #[cfg(feature = "verbose")]
        println!("PID: {} | IO Event Authorized: FD {} | {} on {}", r.process_id, r.file_descriptor, r.method, r.container);

        Ok(Response::new(Grant { file_descriptor: r.file_descriptor }))
    }

    async fn done_io_event(&self, request: Request<Io>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        if let Some(resource_identifier) = {
            let identifiers_map = self.identifiers_map.read().await;
            identifiers_map.get(&(r.process_id.clone(), r.file_descriptor.clone())).cloned()
        } {
            // Obtain MutexGuard on self.containers_states only if an entry is associated with the resource_identifier 
            // and the state of this entry is false.
            // TODO: put it into a function
            if let Some(mut containers_states) = {
                let containers_states = self.containers_states.lock().await;
                if containers_states.get(&resource_identifier) == Some(&false) {
                    Some(containers_states)
                } else { // If the state is true or the entry doesn't exist, return None.
                    None
                }
            } {
                // Give the relay to the next process in the queue
                if let Some(channel) = self.queuing_handler.lock().await
                    .get_mut(&resource_identifier)
                    .and_then(|queue| queue.pop_front())
                {
                    channel.send(()).unwrap();
                } else {
                    containers_states.insert(resource_identifier.clone(), true);
                }
            } else {
                #[cfg(feature = "verbose")]
                println!("Ressource {} is already available", resource_identifier);
            }
        } else {
            #[cfg(feature = "verbose")]
            println!("CT {} is not tracked", r.file_descriptor);
        }

        #[cfg(feature = "verbose")]
        println!("PID: {} | IO Event Done: FD {} | {} on {}", r.process_id, r.file_descriptor, r.method, r.container);     

        Ok(Response::new(Ack{}))
    }
}
