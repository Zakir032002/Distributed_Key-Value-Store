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

// Try leader first (you saw node 3 became leader)
let leader_id = 3;  // In production, client would track this

if let Some(tx) = transport.peers.get(&leader_id) {
    let cmd = Command::Put {
        key: b"username".to_vec(),
        value: b"zakir".to_vec(),
        request_id: 777,
    };

    let result = tx.send(Event::Propose {
        data: serde_json::to_vec(&cmd).unwrap(),
        request_id: 777,
        callback: Box::new(move || {
            println!("✅ [CLIENT] Write committed!");
        }),
    });

    if result.is_ok() {
        println!("📤 Sent proposal to leader (node {})", leader_id);
    }
}

// Wait for commit



// Now READ the value back
println!("\n📖 Reading username...\n");

let cmd_get = Command::Get {
    key: b"username".to_vec(),
    request_id: 888,
};

// Send to leader (node 4 this time, but auto-detect would be better)
let leader_id = 4;  // Update based on who became leader
if let Some(tx) = transport.peers.get(&leader_id) {
    tx.send(Event::Propose {
        data: serde_json::to_vec(&cmd_get).unwrap(),
        request_id: 888,
        callback: Box::new(|| {
            println!("✅ [CLIENT] Read completed!");
        }),
    }).unwrap();
    println!("📤 Sent read request to leader (node {})", leader_id);
}

// IMPORTANT: Wait for the read to complete
println!("\n⏳ Waiting for read to complete...\n");
tokio::time::sleep(Duration::from_secs(2)).await;

println!("\n🎯 All operations complete! Press Ctrl+C to stop\n");

// Keep the cluster running
futures::future::pending::<()>().await;


}
