use serde::{Serialize,Deserialize};
//In a distributed system, data must travel between different machines over the network, and Rust's in-memory objects cannot be sent directly. Serde solves this critical problem
//it will transform it into bytes and send it over the network
#[derive(Debug,Clone,Serialize,Deserialize)]
pub enum Command {
    Put {key : Vec<u8>, value : Vec<u8>},
    Delete {success:bool}
}

#[derive(Debug,Serialize,Deserialize,Clone)]
pub struct Response{
    pub success : bool,
    pub value : Option<Vec<u8>>
}


//These types are used across the Raft core, storage engine, and gRPC layer.