use crate::RaftNode;
use crate::events::Event;

use anyhow::Result;
use tokio::sync::mpsc::{UnboundedReceiver};
use tokio::time::{interval, Duration};
pub struct RaftRuntime{
      node : RaftNode
}
impl RaftRuntime{

}