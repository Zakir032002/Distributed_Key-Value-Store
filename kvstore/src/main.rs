use raftcore::*;
use tokio::sync::mpsc::unbounded_channel;
use std::time::Duration;
use common::Command;
use crate::cluster::ClusterTransport;

#[tokio::main]
async fn main() {
    let node_ids = vec![1, 2, 3, 4, 5];
    let mut transport = ClusterTransport::new();
    let mut handles = vec![];

    println!("Starting 5-node Raft cluster...");

    for id in node_ids.clone() {
        let (tx, rx) = unbounded_channel();

        transport.register(id, tx.clone());

        let node = RaftNode::new(id, node_ids.clone()).unwrap();
        let runtime = RaftRuntime::new(node);
        let t = transport.clone();

        let h = tokio::spawn(async move {
            runtime.start_with_transport(rx, t).await.unwrap();
        });

        handles.push(h);
    }

    println!("Cluster booted. Waiting for election...");
    tokio::time::sleep(Duration::from_millis(1200)).await;

    println!("=> LEADER SHOULD BE ELECTED ABOVE IN LOGS");

    // ---------------------------------------------------------------------
    // TEST CLIENT REQUEST (hit any node; it will drop if not leader)
    // ---------------------------------------------------------------------

    let leader_id = 1; // later we'll auto-detect; for now try ID 1
    let tx = transport.peers.get(&leader_id).unwrap();

    let cmd = Command::Put {
        key: b"username".to_vec(),
        value: b"zakir".to_vec(),
        request_id: 777,
    };

    tx.send(Event::Propose {
        data: serde_json::to_vec(&cmd).unwrap(),
        request_id: 777,
        callback: Box::new(|| println!("[CLIENT] => write committed")),
    }).unwrap();

    println!("Wrote 'username=zakir' to cluster.");

    // ---------------------------------------------------------------------
    // WAIT FOR CTRL-C
    // ---------------------------------------------------------------------
    println!("\nPress Ctrl+C to stop cluster.");
    tokio::signal::ctrl_c().await.unwrap();
}
