use crate::{Event, RaftNode};
use crate::cluster::ClusterTransport;

use anyhow::Result;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{interval, Duration};

pub struct RaftRuntime {
    pub node: RaftNode,
}

impl RaftRuntime {
    pub fn new(node: RaftNode) -> Self {
        Self { node }
    }

    pub async fn start_with_transport(
        mut self,
        mut rx: UnboundedReceiver<Event>,
        mut transport: ClusterTransport,
    ) -> Result<()> {
        let mut ticker = interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    self.node.tick();
                    if let Err(e) = self.node.on_ready(&mut transport) {
                        eprintln!("❌ Node {} on_ready error: {}", self.node.id, e);
                    }
                }

                Some(event) = rx.recv() => {
                    match event {
                        Event::Propose { data, request_id, callback } => {
                            self.node.callbacks.insert(request_id, callback);
                            
                            // DON'T PANIC on error - just log it
                            if let Err(e) = self.node.raw_node.propose(vec![], data) {
                                eprintln!("⚠️  Node {} rejected proposal: {}", self.node.id, e);
                                // Remove callback since proposal failed
                                self.node.callbacks.remove(&request_id);
                            }
                        }
                        Event::Step(msg) => {
                            if let Err(e) = self.node.raw_node.step(msg) {
                                eprintln!("❌ Node {} step error: {}", self.node.id, e);
                            }
                        }
                    }
                    
                    if let Err(e) = self.node.on_ready(&mut transport) {
                        eprintln!("❌ Node {} on_ready error: {}", self.node.id, e);
                    }
                }
            }
        }
    }
}
