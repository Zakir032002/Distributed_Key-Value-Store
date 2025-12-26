use raft::{
      storage::{MemStorage,MemStorageCore}, //core raft in-memory storage
      Config,                               //raft configutration
      prelude::*,                           //common message types
      raw_node::RawNode                     //raft core state machine
};
use slog::{Drain,Logger,o};                 // structured logging
use anyhow::{Ok, Result};                         // easy error

use common::Command;
use serde_json;


// This RaftNode code is the consensus engine that ensures all nodes in your cluster agree on the same sequence of operations, even when nodes crash or networks fail.


pub struct RaftNode{
      pub id : u64,                        // unique node identifier
      pub raw_node : RawNode<MemStorage>,  // the raft concensus state machine
      pub storage : MemStorage,            // Persistent Raft log + hard state
      pub logger  : Logger                 // Structured Logger
}

impl RaftNode {
    pub fn new(id:u64, peers:Vec<u64>)->Result<Self>{
      //setting up the logger
      let decorator = slog_term::TermDecorator::new().build(); //Configures terminal output formatting (colors, timestamps)
      let drain = slog_term::CompactFormat::new(decorator).build().fuse(); //Reduces log verbosity (single-line format)
      let drain = slog_async::Async::new(drain).build().fuse(); // Offloads logging to background thread
      let logger = slog::Logger::root(drain, o!()); // Creates root logger with no additional context

      let mut cfg = Config{
            id,                     //  Node's unique Raft id
            election_tick : 10,     //  Number of ticks before follower becomes candidate and starts election
            heartbeat_tick : 3,     //  Number of ticks between leader heartbeats
            ..Default::default() 
      };

      cfg.validate()?;              //  validates the config like election_tick > heartbeat tick , id != 0 etc..

      let storage = MemStorage::new_with_conf_state((peers.clone(), vec![])); // Creates in-memory storage with initial cluster configuration
      let mut node = RawNode::new(&cfg, storage.clone(), &logger).unwrap(); // 

      Ok(Self { id, raw_node: node, storage, logger })

    }

    // Advances Raft's logical clock by one tick and called by event loop every 100ms
    pub fn tick(&mut self){
      self.raw_node.tick(); // without regular tick calls , the raft node will be in frozen state
    }
      // Follower mode: Increments election timeout counter
      //          : If counter reaches election_tick → become Candidate, start election
      // Leader mode: Increments heartbeat counter
      //          : If counter reaches heartbeat_tick → send heartbeat to all followers
      // Candidate mode: Checks election timeout

    // Feeds Raft messages from other nodes into state machine
    pub fn step(&mut self,message: Message)->Result<()>{
      self.raw_node.step(message)?; //recieves the message and upodates the raft internal state
      Ok(())
    }
      // MsgHeartbeat: Leader→Follower keepalive
      // MsgVote: Candidate→Follower election request
      // MsgAppend: Leader→Follower log replication
      // MsgVoteResp: Follower→Candidate vote response
      // MsgAppendResp: Follower→Leader replication acknowledgment

    //submit client requests
    pub fn propose(&mut self, data : Vec<u8>)->Result<()>{
      self.raw_node.propose(vec![], data)?;
      Ok(())
    }

    pub fn on_ready(&mut self) -> Result<()> {
        if !self.raw_node.has_ready() {
            return Ok(());
        }

        let mut ready = self.raw_node.ready();

        // 1. Send messages to other nodes (handled externally)
        for msg in ready.take_messages() {
            self.handle_message_send(msg);
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
            self.handle_message_send(msg);
        }

        // 7. Advance the Raft node
        let mut light_rd = self.raw_node.advance(ready);

        for msg in light_rd.take_messages() {
            self.handle_message_send(msg);
        }

        for entry in light_rd.take_committed_entries() {
            self.handle_committed(entry)?;
        }

        self.raw_node.advance_apply();

        Ok(())
    }

    fn handle_message_send(&self, _msg: Message) {
        // Transport layer will be inserted here later.
    }

    // Placeholder: KV Store apply layer added next step
    fn handle_committed(&mut self, _entry: Entry) -> Result<()> {
        // We'll decode entry.data & call state machine apply()
        Ok(())
    }

    // Snapshot support 
    fn apply_snapshot(&mut self, snap: &Snapshot) -> Result<()> {
        self.storage.wl().apply_snapshot(snap.clone())?;
        Ok(())
    }


}