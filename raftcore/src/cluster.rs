use crate::{RaftRuntime,Event};
use std::collections::HashMap;
use tokio::sync::mpsc::{UnboundedSender};

#[derive(Clone)]
pub struct ClusterTransport{
      pub peers : HashMap<u64,UnboundedSender<Event>>
}

impl ClusterTransport{
      pub fn new()->Self{
            Self { peers: HashMap::new() }
      }

      pub fn register(&mut self, id: u64, tx : UnboundedSender<Event>){
            self.peers.insert(id, tx);
      }

      pub fn send(&self, mut msg: raft::prelude::Message) {
            if msg.from == 0 {
                  panic!("BUG: attempting to send message with from=0");
            }

            if let Some(tx) = self.peers.get(&msg.to) {
                  if let Err(e) = tx.send(Event::Step(msg)) {
                        eprintln!("transport error: {e}");
                  }
            }
      }

}