use raft::{
    storage::{MemStorage, MemStorageCore}, //core raft in-memory storage
    Config,                               //raft configuration
    prelude::*,                           //common message types
    raw_node::RawNode                     //raft core state machine
};
use slog::{Drain, Logger, o};            // structured logging
use anyhow::{Ok, Result};                // easy error

use common::Command;
use serde_json;
use crate::store::KvStore;
use std::collections::HashMap;
use crate::cluster::ClusterTransport;

// This RaftNode code is the consensus engine that ensures all nodes in your cluster agree on the same sequence of operations, even when nodes crash or networks fail.

pub struct RaftNode{
      pub id : u64,                        // unique node identifier
      pub raw_node : RawNode<MemStorage>,  // the raft consensus state machine
      pub storage : MemStorage,            // Persistent Raft log + hard state
      pub logger  : Logger,                 // Structured Logger
      pub kv_store: KvStore,                // in-memory store for raft
      pub callbacks: HashMap<u64, Box<dyn FnOnce() + Send>>,
}

impl RaftNode {
    pub fn new(id:u64, peers:Vec<u64>)->Result<Self>{
      //setting up the logger
      let decorator = slog_term::TermDecorator::new().build(); 
      let drain = slog_term::CompactFormat::new(decorator).build().fuse();
      let drain = slog_async::Async::new(drain).build().fuse();
      let logger = slog::Logger::root(drain, o!());

      let mut cfg = Config{
            id,                     
            election_tick : 10,    
            heartbeat_tick : 3,
            ..Default::default() 
      };

      cfg.validate()?;
      
                    

      let storage = MemStorage::new_with_conf_state((peers.clone(), vec![])); 
      let mut node = RawNode::new(&cfg, storage.clone(), &logger).unwrap(); 

      if peers.len() == 1 {
        node.campaign().unwrap();
        }

      Ok(Self { id, raw_node: node, storage, logger, kv_store : KvStore::new(), callbacks : HashMap::new() })
    }

    pub fn tick(&mut self){
      self.raw_node.tick(); 
    }

    pub fn step(&mut self,message: Message)->Result<()>{
      self.raw_node.step(message)?; 
      Ok(())
    }

    pub fn propose(&mut self, data : Vec<u8>)->Result<()>{
      self.raw_node.propose(vec![], data)?;
      Ok(())
    }

    // NOW ACCEPTS TRANSPORT
    pub fn on_ready(&mut self, transport: &mut ClusterTransport) -> Result<()> {
        if !self.raw_node.has_ready() {
            return Ok(());
        }

        let mut ready = self.raw_node.ready();

        // 1. Send messages to other nodes (handled externally)
        for msg in ready.take_messages() {
            self.handle_message_send(msg, transport);
        }

        // 2. Handle snapshot
        if !ready.snapshot().is_empty() {
            self.apply_snapshot(&ready.snapshot())?;
        }

        // 3. Apply committed entries to state machine (KV Store)
        for entry in ready.take_committed_entries() {
            self.handle_committed(entry)?;
        }

        // 4. Append entries to log
        if !ready.entries().is_empty() {
            self.storage.wl().append(ready.entries())?;
        }

        // 5. Apply HardState change (new leader, commit index, etc.)
        if let Some(hs) = ready.hs() {
            self.storage.wl().set_hardstate(hs.clone());
        }

        // 6. Persisted messages (after hardstate + entries)
        for msg in ready.take_persisted_messages() {
            self.handle_message_send(msg, transport);
        }

        // 7. Advance the Raft node
        let mut light_rd = self.raw_node.advance(ready);

        for msg in light_rd.take_messages() {
            self.handle_message_send(msg, transport);
        }

        for entry in light_rd.take_committed_entries() {
            self.handle_committed(entry)?;
        }

        self.raw_node.advance_apply();

        Ok(())
    }

    // IMPORTANT: FIXED SIGNATURE (NOW CORRECT)
    pub fn handle_message_send(&self, msg: Message, transport: &ClusterTransport) {
        transport.send(msg);
    }

    fn handle_committed(&mut self, entry: Entry) -> Result<()> {
        if entry.data.is_empty() {
            return Ok(());
        }

        if entry.get_entry_type() == EntryType::EntryNormal {
            let cmd: Command = serde_json::from_slice(&entry.data)?;
            self.apply_normal(cmd)?
        }

        Ok(())
    }

    fn apply_snapshot(&mut self, snap: &Snapshot) -> Result<()> {
        self.storage.wl().apply_snapshot(snap.clone())?;
        Ok(())
    }

    fn apply_normal(&mut self,cmd:Command)->Result<()>{
        match cmd{
            Command::Put { key, value, request_id } =>{
                self.kv_store.put(key, value);
                if let Some(cb) = self.callbacks.remove(&request_id) {
                    cb();
                }
            }
            Command::Delete { request_id,key }=>{
                self.kv_store.delete(&key);
                if let Some(cb) = self.callbacks.remove(&request_id) {
                    cb();
                }
            }
        }
        Ok(())
    }

    pub fn propose_with_callback(&mut self,cmd: Command,request_id: u64,callback: Box<dyn FnOnce() + Send>) -> Result<()> {
        let data = serde_json::to_vec(&cmd)?;
        self.callbacks.insert(request_id, callback);
        self.raw_node.propose(vec![], data)?;
        Ok(())
    }
}
