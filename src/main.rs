use zmq::Context;
use serde::{Serialize, Deserialize};

use std::{thread, time};

#[derive(Debug, Serialize, Deserialize)]
enum EventType {
    IO,
    CT,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    event_type: EventType,
    // common fields
    file_descriptor: i32,
    container: String,
    // IO fields
    method: Option<String>,
    // CT fields
    ressource_identifier: Option<String>, // file path, host socket
    input: Option<bool>, // eg. read privilege on a file
    output: Option<bool>, // eg. write privilege on a file
    remote: Option<bool>, // eg. communication channel trough network
}

fn main() {
    let context = Context::new();
    let socket = context.socket(zmq::REP).unwrap();
    assert!(socket.bind("tcp://*:5555").is_ok());

    loop {
        let buffer = socket.recv_bytes(0).unwrap();
        let event: Event = rmp_serde::from_slice(&buffer).unwrap();
        match event.event_type {
            EventType::IO => println!("IO Event: FD {} | {} on {}", event.file_descriptor, event.method.unwrap(), event.container), 
            EventType::CT => println!("CT Event: FD {} | IN {} | OUT {} | REMOTE {} | {} @ {}", event.file_descriptor, event.input.unwrap(), event.output.unwrap(), event.remote.unwrap(), event.container, event.ressource_identifier.unwrap()),
        };
        let _ = socket.send("ack", 0);
    }
}