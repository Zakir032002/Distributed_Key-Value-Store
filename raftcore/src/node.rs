use raft::{
    storage::{MemStorage, MemStorageCore},
    Config,
    prelude::*,
    raw_node::RawNode
};
use slog::{Drain, Logger, o};
use anyhow::Result;
use common::Command;
use serde_json;
use crate::store::KvStore;
use std::collections::HashMap;
use crate::cluster::ClusterTransport;

pub struct RaftNode {
    pub id: u64,
    pub raw_node: RawNode<MemStorage>,
    pub storage: MemStorage,
    pub logger: Logger,
    pub kv_store: KvStore,
    pub callbacks: HashMap<u64, Box<dyn FnOnce() + Send>>,
}

impl RaftNode {
    pub fn new(id: u64, peers: Vec<u64>) -> Result<Self> {
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain).build().fuse();
        let logger = slog::Logger::root(drain, o!());

        let mut cfg = Config {
            id,
            election_tick: 10,
            heartbeat_tick: 3,
            max_size_per_msg: 1024 * 1024,
            max_inflight_msgs: 256,
            check_quorum: true,
            pre_vote: true,
            ..Default::default()
        };

        cfg.validate()?;

        let storage = MemStorage::new_with_conf_state((peers.clone(), vec![]));
        let mut node = RawNode::new(&cfg, storage.clone(), &logger)?;

        if peers.len() == 1 {
            node.campaign()?;
        }

        Ok(Self {
            id,
            raw_node: node,
            storage,
            logger,
            kv_store: KvStore::new(),
            callbacks: HashMap::new(),
        })
    }

    pub fn tick(&mut self) {
        self.raw_node.tick();
    }

    pub fn step(&mut self, message: Message) -> Result<()> {
        self.raw_node.step(message)?;
        Ok(())
    }

    pub fn propose(&mut self, data: Vec<u8>) -> Result<()> {
        match self.raw_node.propose(vec![], data) {
            Ok(_) => Ok(()),
            Err(e) => {
                eprintln!("⚠️ Node {} propose failed: {} (probably not leader)", self.id, e);
                Err(anyhow::anyhow!(e))
            }
        }
    }

    pub fn propose_with_callback(
        &mut self,
        cmd: Command,
        request_id: u64,
        callback: Box<dyn FnOnce() + Send>
    ) -> Result<()> {
        let data = serde_json::to_vec(&cmd)?;
        self.callbacks.insert(request_id, callback);
        match self.raw_node.propose(vec![], data) {
            Ok(_) => Ok(()),
            Err(e) => {
                self.callbacks.remove(&request_id);
                Err(anyhow::anyhow!(e))
            }
        }
    }

    // ✅ CORRECTED: Safety logic (Persist -> Send -> Apply)
    pub fn on_ready(&mut self, transport: &mut ClusterTransport) -> Result<()> {
        if !self.raw_node.has_ready() {
            return Ok(());
        }

        let mut ready = self.raw_node.ready();

        // 1. SAVE to Storage (HardState + Entries)
        if !ready.entries().is_empty() {
            self.storage.wl().append(ready.entries())?;
        }

        if let Some(hs) = ready.hs() {
            self.storage.wl().set_hardstate(hs.clone());
        }

        if !ready.snapshot().is_empty() {
            self.storage.wl().apply_snapshot(ready.snapshot().clone())?;
        }

        // 2. Send Messages
        for msg in ready.take_messages() {
            self.handle_message_send(msg, transport);
        }

        // 3. Apply committed entries
        for entry in ready.take_committed_entries() {
            if let Err(e) = self.handle_committed(entry) {
                eprintln!("❌ Node {} failed to apply entry: {}", self.id, e);
            }
        }

        // 4. Send persisted messages
        for msg in ready.take_persisted_messages() {
            self.handle_message_send(msg, transport);
        }

        // 5. Advance
        let mut light_rd = self.raw_node.advance(ready);

        for msg in light_rd.take_messages() {
            self.handle_message_send(msg, transport);
        }

        for entry in light_rd.take_committed_entries() {
            if let Err(e) = self.handle_committed(entry) {
                eprintln!("❌ Node {} failed to apply entry: {}", self.id, e);
            }
        }

        self.raw_node.advance_apply();

        Ok(())
    }

    pub fn handle_message_send(&self, msg: Message, transport: &ClusterTransport) {
        transport.send(msg);
    }

    fn handle_committed(&mut self, entry: Entry) -> Result<()> {
        if entry.data.is_empty() {
            return Ok(());
        }

        match entry.get_entry_type() {
            EntryType::EntryNormal => {
                let cmd: Command = serde_json::from_slice(&entry.data)?;
                self.apply_normal(cmd)?;
            }
            EntryType::EntryConfChange | EntryType::EntryConfChangeV2 => {
                eprintln!("⚠️ Config change entry ignored (not implemented)");
            }
        }

        Ok(())
    }

    // ✅ ADDED: Get handling and Better Logging
    fn apply_normal(&mut self, cmd: Command) -> Result<()> {
        match cmd {
            Command::Put { key, value, request_id } => {
                // Clone keys for printing before moving them into the store
                let key_str = String::from_utf8_lossy(&key).to_string();
                let val_str = String::from_utf8_lossy(&value).to_string();

                self.kv_store.put(key, value);
                
                println!("✅ [Node {}] Applied PUT: key={}, value={}", self.id, key_str, val_str);
                
                if let Some(cb) = self.callbacks.remove(&request_id) {
                    cb();
                }
            }
            Command::Get { key, request_id } => {
                // Reading via Raft log (Linearizable Read)
                if let Some(val) = self.kv_store.get(&key) {
                    println!("✅ [Node {}] Applied GET: key={}, value={}", 
                        self.id, 
                        String::from_utf8_lossy(&key), 
                        String::from_utf8_lossy(&val)
                    );
                } else {
                    println!("⚠️ [Node {}] Applied GET: key={} (NOT FOUND)", 
                        self.id, 
                        String::from_utf8_lossy(&key)
                    );
                }
                
                if let Some(cb) = self.callbacks.remove(&request_id) {
                    cb();
                }
            }
            Command::Delete { key, request_id } => {
                let key_str = String::from_utf8_lossy(&key).to_string();
                
                self.kv_store.delete(&key);
                
                println!("✅ [Node {}] Applied DELETE: key={}", self.id, key_str);
                
                if let Some(cb) = self.callbacks.remove(&request_id) {
                    cb();
                }
            }
        }
        Ok(())
    }
}