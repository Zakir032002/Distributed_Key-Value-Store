use crate::RaftNode;
use crate::events::Event;

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

    pub async fn start(mut self, mut rx: UnboundedReceiver<Event>) -> Result<()> {
        let mut ticker = interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                // Tick every 100ms
                _ = ticker.tick() => {
                    self.node.tick();
                    self.node.on_ready()?;
                }

                // Receive events
                Some(event) = rx.recv() => {
                    match event {
                        Event::Propose { data, request_id, callback } => {
                            self.node.callbacks.insert(request_id, callback);
                            self.node.raw_node.propose(vec![], data)?;
                        }

                        Event::Step(msg) => {
                            self.node.raw_node.step(msg)?;
                        }
                    }

                    self.node.on_ready()?;
                }
            }
        }
    }
}
