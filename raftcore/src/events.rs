use raft::prelude::*;

pub enum Event {
    // Client wants to PUT/DELETE
    Propose {
        data: Vec<u8>,
        // request id used to track callbacks
        request_id: u64,
        callback: Box<dyn FnOnce() + Send>,
    },

    // Incoming raft message (multi-node will use this)
    Step(Message),
}
