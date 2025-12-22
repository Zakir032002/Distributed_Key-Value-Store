use serde::{Deserialize, Serialize};

#[derive(Debug,Clone,Serialize)]
pub struct NodeConfig{
      pub node_id : u64,
      pub address : String,
      pub peers : Vec<String>,
      pub data_dir : String
}