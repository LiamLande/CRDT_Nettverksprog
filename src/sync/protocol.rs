use crate::sync::Operation;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ClientWireMessage {
    Hello {
        replica_id: String,
        seen_operations: Vec<String>,
    },
    Operation(Operation),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ServerWireMessage {
    Info { message: String },
    Operation(Operation),
}
