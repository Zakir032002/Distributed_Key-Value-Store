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

    pub async fn start_with_transport(mut self,mut rx: UnboundedReceiver<Event>,transport: ClusterTransport) -> Result<()> {


        let mut ticker = interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                // TICK → drives election & heartbeat
                _ = ticker.tick() => {
                    self.node.tick();
                    self.node.on_ready(&transport)?;
                }

                // INCOMING CLIENT / RAFT EVENTS
                Some(event) = rx.recv() => {
                    match event {
                        Event::Propose { data, request_id, callback } => {
                            // client command
                            self.node.callbacks.insert(request_id, callback);
                            self.node.raw_node.propose(vec![], data)?;
                        }
                        Event::Step(msg) => {
                            // raft protocol messages
                            self.node.raw_node.step(msg)?;
                        }
                    }
                    self.node.on_ready(&transport)?;
                }
            }
        }
    }
}
