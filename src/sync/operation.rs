use crate::domain::{BetKind, RoundPhase};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    pub op_id: String,
    pub replica_id: String,
    pub lamport_time: u64,
    #[serde(flatten)]
    pub kind: OperationKind,
}

impl Operation {
    pub fn new(replica_id: impl Into<String>, lamport_time: u64, kind: OperationKind) -> Self {
        let replica_id = replica_id.into();
        Self {
            op_id: format!("{replica_id}-{lamport_time}-{}", Uuid::new_v4().simple()),
            replica_id,
            lamport_time,
            kind,
        }
    }

    pub fn fixed(
        op_id: impl Into<String>,
        replica_id: impl Into<String>,
        lamport_time: u64,
        kind: OperationKind,
    ) -> Self {
        Self {
            op_id: op_id.into(),
            replica_id: replica_id.into(),
            lamport_time,
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum OperationKind {
    CreateTable {
        table_id: String,
        name: String,
    },
    JoinTable {
        table_id: String,
        player_id: String,
        display_name: String,
        starting_chips: u64,
        initial_rights: BTreeMap<String, u64>,
    },
    LeaveTable {
        table_id: String,
        player_id: String,
    },
    CreateRound {
        round_id: String,
        dealer_id: String,
    },
    OpenBetting {
        round_id: String,
    },
    PlaceBet {
        bet_id: String,
        player_id: String,
        round_id: String,
        amount: u64,
        bet_kind: BetKind,
        origin_round_phase: RoundPhase,
    },
    CloseBetting {
        round_id: String,
    },
    ResolveRound {
        round_id: String,
        dice: u8,
    },
    ApplyPayout {
        payout_id: String,
        player_id: String,
        round_id: String,
        amount: u64,
    },
    TransferSpendingRights {
        player_id: String,
        from_replica: String,
        to_replica: String,
        amount: u64,
    },
}
