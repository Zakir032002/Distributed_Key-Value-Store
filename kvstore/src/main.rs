use raftcore::{RaftNode, RaftRuntime, Event};
use common::Command;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    println!("raft_id: 1");

    let (tx, rx) = unbounded_channel::<Event>();
    let id = 1;
    let peers = vec![1]; // single-node cluster

    let node = RaftNode::new(id, peers).unwrap();
    let runtime = RaftRuntime::new(node);

    tokio::spawn(async move {
        runtime.start(rx).await.unwrap();
    });

    // -----------------------------
    // WAIT FOR LEADER ELECTION
    // -----------------------------
    println!("Waiting for leader election...");
    sleep(Duration::from_millis(500)).await;

    // -----------------------------
    // PROPOSE COMMAND
    // -----------------------------
    let cmd = Command::Put {
        key: b"hello".to_vec(),
        value: b"world".to_vec(),
        request_id: 1,
    };

    let bytes = serde_json::to_vec(&cmd).unwrap();

    tx.send(Event::Propose {
        data: bytes,
        request_id: 1,
        callback: Box::new(|| {
            println!("PUT committed!");
        }),
    }).unwrap();

    // Keep running
    tokio::signal::ctrl_c().await.unwrap();
}
