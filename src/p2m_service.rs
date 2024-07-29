use p2m::{LocalCt, RemoteCt, IoInfo, IoResult, Grant, Ack, p2m_server::P2m};
use tonic::{Request, Response, Status};
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex, RwLock};
use std::collections::{HashMap, VecDeque};

use crate::containers::ContainersManager;

pub mod p2m {
    tonic::include_proto!("trace2e_api");
    pub const FILE_DESCRIPTOR_SET: &[u8] = include_bytes!("../target/trace2e_api_descriptor.bin");
}

#[derive(Debug)]
pub struct P2mService {
    containers_manager: Arc<Mutex<ContainersManager>>,
    identifiers_map: Arc<RwLock<HashMap<(u32, i32), String>>>,
    queuing_handler: Arc<Mutex<HashMap<String, VecDeque<oneshot::Sender<()>>>>>,
}

impl P2mService {
    pub fn new() -> Self {
        P2mService {
            containers_manager: Arc::new(Mutex::new(ContainersManager::default())),
            identifiers_map: Arc::new(RwLock::new(HashMap::new())),
            queuing_handler: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    async fn get_resource_identifier(&self, process_id: u32, file_descriptor: i32) -> Option<String> {
        self.identifiers_map.read()
            .await
            .get(&(process_id, file_descriptor)).cloned()
    }

    async fn container_reservation(&self, resource_identifier: String, process_id: u32) {
        match self.containers_manager.lock()
            .await
            .try_reservation(resource_identifier.clone())
        {
            Ok(true) => (),
            Ok(false) => self.wait_container_release(resource_identifier, process_id).await,
            Err(msg) => eprintln!("{}", msg)
        }
    }

    async fn wait_container_release(&self, resource_identifier: String, _process_id: u32) {
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
            println!("-> Wait a bit PID {}", _process_id);
            
            rx.await.unwrap();

            // CT is now reserved for the current process
            #[cfg(feature = "verbose")]
            println!("<- Now it's your turn PID {}", _process_id);
    }

    async fn container_release(&self, resource_identifier: String) {
        if let Some(channel) = self.queuing_handler.lock().await
            .get_mut(&resource_identifier)
            .and_then(|queue| queue.pop_front())
        {
            channel.send(()).unwrap();
        } else {
            match self.containers_manager.lock()
                .await
                .try_release(resource_identifier)
            {
                Ok(()) => (),
                Err(msg) => eprintln!("{}", msg)
            }
        }
    }
}

#[tonic::async_trait]
impl P2m for P2mService {
    async fn local_enroll(&self, request: Request<LocalCt>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        #[cfg(feature = "verbose")]
        println!("PID: {} | CT Event Notified: FD {} | {}", r.process_id.clone(),  r.file_descriptor.clone(), r.path.clone()); 

        self.containers_manager.lock()
            .await
            .register(r.path.clone());
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
            self.container_reservation(resource_identifier.clone(), r.process_id).await; 
        } else {
            #[cfg(feature = "verbose")]
            println!("CT {} is not tracked", r.file_descriptor);
        }
        
        #[cfg(feature = "verbose")]
        println!("PID: {} | IO Event Authorized: FD {}", r.process_id, r.file_descriptor);

        Ok(Response::new(Grant { id: 0 })) // TODO : implement grant_id
    }

    async fn io_report(&self, request: Request<IoResult>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        if let Some(resource_identifier) = self.get_resource_identifier(r.process_id, r.file_descriptor).await {
            self.container_release(resource_identifier.clone()).await;
        } else {
            #[cfg(feature = "verbose")]
            println!("CT {} is not tracked", r.file_descriptor);
        }

        #[cfg(feature = "verbose")]
        println!("PID: {} | IO Event Done: FD {}", r.process_id, r.file_descriptor);     

        Ok(Response::new(Ack{}))
    }
}
