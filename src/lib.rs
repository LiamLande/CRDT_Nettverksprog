pub mod client;
pub mod crdt;
pub mod domain;
pub mod frontend;
pub mod server;
pub mod sync;

pub use client::LocalClient;
pub use domain::{BetKind, BetSyncState, RoundPhase};
pub use sync::{Operation, OperationKind};
