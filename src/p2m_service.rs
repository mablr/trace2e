use p2m::{LocalCt, RemoteCt, IoInfo, IoResult, Grant, Ack, p2m_server::P2m};
use tonic::{Request, Response, Status};
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex, RwLock};
use std::collections::{HashMap, VecDeque};

use crate::container::ContainerManager;

pub mod p2m {
    tonic::include_proto!("trace2e_api");
    pub const FILE_DESCRIPTOR_SET: &[u8] = include_bytes!("../target/trace2e_api_descriptor.bin");
}

#[derive(Debug)]
pub struct P2mService {
    containers_states: Arc<Mutex<HashMap<String, ContainerManager>>>,
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

    async fn create_local_container(&self, path: String) {
        /*
        Adding CT to the tracklist
        If it is already in the hashmap, do nothing to avoid erasing of the previous container state.
        */
        let mut containers_states = self.containers_states.lock().await;
        if containers_states.contains_key(&path) == false {
            containers_states.insert(path, ContainerManager::default());
        }
    }

    async fn get_resource_identifier(&self, process_id: u32, file_descriptor: i32) -> Option<String> {
        let identifiers_map = self.identifiers_map.read().await;
        identifiers_map.get(&(process_id, file_descriptor)).cloned()
    }

    async fn try_container_reservation(&self, resource_identifier: String) -> bool {
        let mut containers_states = self.containers_states.lock().await;
        if containers_states.get(&resource_identifier).is_some_and(|container| container.is_available()) {
            let container = containers_states.get_mut(&resource_identifier).unwrap();
            container.set_availability(false);
            true
        } else { // If the state is not expected or the entry doesn't exist, return None.
            false
        }
        
    }

    async fn container_release (&self, resource_identifier: String) -> bool {
        let mut containers_states = self.containers_states.lock().await;
        if containers_states.get(&resource_identifier).is_some_and(|container| container.is_available() == false) {
            if let Some(channel) = self.queuing_handler.lock().await
            .get_mut(&resource_identifier)
            .and_then(|queue| queue.pop_front())
            {
                channel.send(()).unwrap();
            } else {
                let container = containers_states.get_mut(&resource_identifier).unwrap();
                container.set_availability(true);
            }
            
            true
        } else {
            false
        }
    }
}

#[tonic::async_trait]
impl P2m for P2mService {
    async fn local_enroll(&self, request: Request<LocalCt>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        #[cfg(feature = "verbose")]
        println!("PID: {} | CT Event Notified: FD {} | {}", r.process_id,  r.file_descriptor, r.path); 

        self.create_local_container(r.path.clone()).await;
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
            identifiers_map.insert((r.process_id.clone(), r.file_descriptor.clone()), r.path.clone());
        }
        Ok(Response::new(Ack {}))
    }

    async fn remote_enroll(&self, request: Request<RemoteCt>) -> Result<Response<Ack>, Status> {
        let _ = request.into_inner();

        Ok(Response::new(Ack {}))
    }

    async fn io_request(&self, request: Request<IoInfo>) -> Result<Response<Grant>, Status> {
        let r = request.into_inner();

        #[cfg(feature = "verbose")]
        println!("PID: {} | IO Event Requested: FD {}", r.process_id, r.file_descriptor);

        if let Some(resource_identifier) = self.get_resource_identifier(r.process_id, r.file_descriptor).await {
            // Obtain MutexGuard on self.containers_states only if an entry is associated with the resource_identifier 
            // and the state of this entry is true.
            // TODO: put it into a function
            if self.try_container_reservation(resource_identifier.clone()).await == false {
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
        println!("PID: {} | IO Event Authorized: FD {}", r.process_id, r.file_descriptor);

        Ok(Response::new(Grant { id: 0 })) // TODO : to be updated
    }

    async fn io_report(&self, request: Request<IoResult>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        if let Some(resource_identifier) = self.get_resource_identifier(r.process_id, r.file_descriptor).await {
            // Obtain MutexGuard on self.containers_states only if an entry is associated with the resource_identifier 
            // and the state of this entry is false.
            // TODO: put it into a function
            if self.container_release(resource_identifier.clone()).await == false {
                #[cfg(feature = "verbose")]
                println!("Ressource {} is already available", resource_identifier);
            }
        } else {
            #[cfg(feature = "verbose")]
            println!("CT {} is not tracked", r.file_descriptor);
        }

        #[cfg(feature = "verbose")]
        println!("PID: {} | IO Event Done: FD {}", r.process_id, r.file_descriptor);     

        Ok(Response::new(Ack{}))
    }
}
