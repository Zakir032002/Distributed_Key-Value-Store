use crate::Event;
use std::collections::HashMap;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone)]
pub struct ClusterTransport {
    pub peers: HashMap<u64, UnboundedSender<Event>>,
}

impl ClusterTransport {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    pub fn register(&mut self, id: u64, tx: UnboundedSender<Event>) {
        self.peers.insert(id, tx);
    }

    pub fn send(&self, msg: raft::prelude::Message) {
        if let Some(tx) = self.peers.get(&msg.to) {
            let _ = tx.send(Event::Step(msg));
        } else {
            // This should NEVER happen if setup is correct
            eprintln!("❌ CRITICAL: No route to node {} (from attempted by node)", msg.to);
        }
    }
}
