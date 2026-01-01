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

    println!("🚀 Starting 5-node Raft cluster...\n");

    // Register ALL nodes FIRST
    let mut channels = vec![];
    for id in node_ids.clone() {
        let (tx, rx) = unbounded_channel();
        transport.register(id, tx.clone());
        channels.push((id, rx));
    }

    // NOW start all nodes with fully-initialized transport
    for (id, rx) in channels {
        let node = RaftNode::new(id, node_ids.clone())
            .expect("Failed to create Raft node");
        
        let runtime = RaftRuntime::new(node);
        let t = transport.clone();  // All peers already registered

        let h = tokio::spawn(async move {
            println!("✅ Node {} started", id);
            if let Err(e) = runtime.start_with_transport(rx, t).await {
                eprintln!("❌ Node {} error: {}", id, e);
            }
        });

        handles.push(h);
    }

    println!("⏳ Waiting for leader election (3 seconds)...\n");
    tokio::time::sleep(Duration::from_secs(3)).await;

    println!("\n📝 Attempting write: username=zakir\n");

for node_id in node_ids.clone() {
    if let Some(tx) = transport.peers.get(&node_id) {
        let cmd = Command::Put {
            key: b"username".to_vec(),
            value: b"zakir".to_vec(),
            request_id: 777,
        };

        let result = tx.send(Event::Propose {
            data: serde_json::to_vec(&cmd).unwrap(),
            request_id: 777,
            callback: Box::new(move || {
                println!("✅ [CLIENT] Write committed via node {}!", node_id);
            }),
        });

        if result.is_ok() {
            println!("📤 Sent proposal to node {}", node_id);
            break;
        }
    }
}

// Wait for commit
tokio::time::sleep(Duration::from_millis(500)).await;

// Now READ the value back
println!("\n📖 Reading username...\n");

let cmd_get = Command::Get {
    key: b"username".to_vec(),
    request_id: 888,
};

if let Some(tx) = transport.peers.get(&1) {
    tx.send(Event::Propose {
        data: serde_json::to_vec(&cmd_get).unwrap(),
        request_id: 888,
        callback: Box::new(|| {
            println!("✅ [CLIENT] Read completed!");
        }),
    }).unwrap();
}

println!("\n🎯 Press Ctrl+C to stop\n");

}
