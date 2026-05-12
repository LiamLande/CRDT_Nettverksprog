use crate::domain::{BetKind, BetSyncState, RoundPhase};
use crate::sync::{LamportClock, Operation, OperationKind, OperationLog};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LocalClient {
    pub replica_id: String,
    pub clock: LamportClock,
    pub log: OperationLog,
    pub state: BetSyncState,
    unsynced_operations: Vec<Operation>,
    online: bool,
}

impl LocalClient {
    pub fn new(replica_id: impl Into<String>) -> Self {
        Self {
            replica_id: replica_id.into(),
            clock: LamportClock::new(),
            log: OperationLog::default(),
            state: BetSyncState::default(),
            unsynced_operations: Vec::new(),
            online: true,
        }
    }

    pub fn set_online(&mut self, online: bool) {
        self.online = online;
    }

    pub fn is_online(&self) -> bool {
        self.online
    }

    pub fn queue_unsynced(&mut self, operation: Operation) {
        if !self
            .unsynced_operations
            .iter()
            .any(|queued| queued.op_id == operation.op_id)
        {
            self.unsynced_operations.push(operation);
        }
    }

    pub fn take_unsynced(&mut self) -> Vec<Operation> {
        std::mem::take(&mut self.unsynced_operations)
    }

    pub fn ingest(&mut self, operation: Operation) -> bool {
        self.clock.observe(operation.lamport_time);
        let inserted = self.log.insert(operation);
        if inserted {
            self.rebuild_state();
        }
        inserted
    }

    pub fn ingest_many(&mut self, operations: impl IntoIterator<Item = Operation>) -> usize {
        let mut inserted = 0;
        for operation in operations {
            if self.ingest(operation) {
                inserted += 1;
            }
        }
        inserted
    }

    pub fn merge_from(&mut self, other: &LocalClient) -> usize {
        self.ingest_many(other.log.operations.clone())
    }

    pub fn create_table(&mut self, name: impl Into<String>) -> Operation {
        self.record(OperationKind::CreateTable {
            table_id: "main".to_string(),
            name: name.into(),
        })
    }

    pub fn join_table(
        &mut self,
        player_id: impl Into<String>,
        display_name: impl Into<String>,
        starting_chips: u64,
    ) -> Operation {
        let mut initial_rights = BTreeMap::new();
        initial_rights.insert(self.replica_id.clone(), starting_chips);
        self.join_table_with_rights(player_id, display_name, starting_chips, initial_rights)
    }

    pub fn join_table_with_rights(
        &mut self,
        player_id: impl Into<String>,
        display_name: impl Into<String>,
        starting_chips: u64,
        initial_rights: BTreeMap<String, u64>,
    ) -> Operation {
        self.record(OperationKind::JoinTable {
            table_id: "main".to_string(),
            player_id: player_id.into(),
            display_name: display_name.into(),
            starting_chips,
            initial_rights,
        })
    }

    pub fn leave_table(&mut self, player_id: impl Into<String>) -> Operation {
        self.record(OperationKind::LeaveTable {
            table_id: "main".to_string(),
            player_id: player_id.into(),
        })
    }

    pub fn create_round(
        &mut self,
        round_id: impl Into<String>,
        dealer_id: impl Into<String>,
    ) -> Operation {
        self.record(OperationKind::CreateRound {
            round_id: round_id.into(),
            dealer_id: dealer_id.into(),
        })
    }

    pub fn open_betting(&mut self, round_id: impl Into<String>) -> Operation {
        self.record(OperationKind::OpenBetting {
            round_id: round_id.into(),
        })
    }

    pub fn start_round(
        &mut self,
        round_id: impl Into<String>,
        dealer_id: impl Into<String>,
    ) -> Vec<Operation> {
        let round_id = round_id.into();
        let dealer_id = dealer_id.into();
        vec![
            self.create_round(round_id.clone(), dealer_id),
            self.open_betting(round_id),
        ]
    }

    pub fn place_bet(
        &mut self,
        player_id: impl Into<String>,
        round_id: impl Into<String>,
        amount: u64,
        bet_kind: BetKind,
    ) -> Result<Operation, String> {
        let player_id = player_id.into();
        let round_id = round_id.into();

        if !self.state.players.contains(&player_id) {
            return Err(format!("player {player_id} has not joined"));
        }

        let Some(origin_round_phase) = self.state.round_phase(&round_id) else {
            return Err(format!("round {round_id} does not exist"));
        };
        if origin_round_phase != RoundPhase::BettingOpen {
            return Err(format!("round {round_id} is not open for betting"));
        }

        let available = self
            .state
            .available_rights(&player_id, self.replica_id.as_str());
        if available < amount as i64 {
            return Err(format!(
                "insufficient spending rights on replica {}: have {available}, need {amount}",
                self.replica_id
            ));
        }

        Ok(self.record(OperationKind::PlaceBet {
            bet_id: format!("bet-{}", Uuid::new_v4().simple()),
            player_id,
            round_id,
            amount,
            bet_kind,
            origin_round_phase,
        }))
    }

    pub fn close_betting(&mut self, round_id: impl Into<String>) -> Result<Operation, String> {
        let round_id = round_id.into();
        if self.state.round_phase(&round_id) != Some(RoundPhase::BettingOpen) {
            return Err(format!("round {round_id} is not open"));
        }
        Ok(self.record(OperationKind::CloseBetting { round_id }))
    }

    pub fn resolve_round(
        &mut self,
        round_id: impl Into<String>,
        dice: u8,
    ) -> Result<Vec<Operation>, String> {
        let round_id = round_id.into();
        if !(1..=6).contains(&dice) {
            return Err("dice result must be 1 through 6".to_string());
        }
        if self.state.round_phase(&round_id) != Some(RoundPhase::BettingClosed) {
            return Err(format!("round {round_id} is not closed"));
        }

        let mut operations = vec![self.record(OperationKind::ResolveRound {
            round_id: round_id.clone(),
            dice,
        })];

        let bets: Vec<_> = self
            .state
            .visible_bets()
            .into_iter()
            .filter(|bet| bet.round_id == round_id && bet.kind.wins(dice))
            .collect();

        for bet in bets {
            operations.push(self.record(OperationKind::ApplyPayout {
                payout_id: format!("payout:{}:{}", round_id, bet.id),
                player_id: bet.player_id,
                round_id: round_id.clone(),
                amount: bet.amount * bet.kind.multiplier(),
            }));
        }

        Ok(operations)
    }

    pub fn transfer_spending_rights(
        &mut self,
        player_id: impl Into<String>,
        to_replica: impl Into<String>,
        amount: u64,
    ) -> Result<Operation, String> {
        let player_id = player_id.into();
        let to_replica = to_replica.into();
        let available = self
            .state
            .available_rights(&player_id, self.replica_id.as_str());
        if available < amount as i64 {
            return Err(format!(
                "insufficient spending rights on replica {}: have {available}, need {amount}",
                self.replica_id
            ));
        }
        Ok(self.record(OperationKind::TransferSpendingRights {
            player_id,
            from_replica: self.replica_id.clone(),
            to_replica,
            amount,
        }))
    }

    pub fn operation_by_id(&self, op_id: &str) -> Option<Operation> {
        self.log
            .operations
            .iter()
            .find(|operation| operation.op_id == op_id)
            .cloned()
    }

    pub fn record_operation_kind(&mut self, kind: OperationKind) -> Operation {
        self.record(kind)
    }

    fn record(&mut self, kind: OperationKind) -> Operation {
        let lamport_time = self.clock.tick();
        let operation = Operation::new(self.replica_id.clone(), lamport_time, kind);
        self.log.insert(operation.clone());
        self.rebuild_state();
        if !self.online {
            self.queue_unsynced(operation.clone());
        }
        operation
    }

    fn rebuild_state(&mut self) {
        self.state = BetSyncState::from_operations(&self.log.operations);
    }
}
