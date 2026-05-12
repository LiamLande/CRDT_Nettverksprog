use crate::crdt::{BoundedCounter, LwwRegister, OrSet, PNCounter};
use crate::domain::{Bet, Round, RoundPhase};
use crate::sync::{Operation, OperationKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BetSyncState {
    pub table_name: LwwRegister<Option<String>>,
    pub players: OrSet<String>,
    pub player_names: BTreeMap<String, LwwRegister<String>>,
    pub rounds: OrSet<String>,
    pub round_views: BTreeMap<String, Round>,
    pub bets: OrSet<Bet>,
    pub balances: BTreeMap<String, PNCounter>,
    pub spending_rights: BTreeMap<String, BoundedCounter>,
    pub applied_payouts: BTreeSet<String>,
    pub rejected_operations: BTreeMap<String, String>,
}

impl Default for BetSyncState {
    fn default() -> Self {
        Self {
            table_name: LwwRegister::new(None, 0, ""),
            players: OrSet::default(),
            player_names: BTreeMap::new(),
            rounds: OrSet::default(),
            round_views: BTreeMap::new(),
            bets: OrSet::default(),
            balances: BTreeMap::new(),
            spending_rights: BTreeMap::new(),
            applied_payouts: BTreeSet::new(),
            rejected_operations: BTreeMap::new(),
        }
    }
}

impl BetSyncState {
    pub fn from_operations(operations: &[Operation]) -> Self {
        let mut state = Self::default();
        let mut sorted = operations.to_vec();
        sorted.sort_by(|left, right| {
            (left.lamport_time, &left.replica_id, &left.op_id).cmp(&(
                right.lamport_time,
                &right.replica_id,
                &right.op_id,
            ))
        });

        for operation in &sorted {
            state.apply(operation);
        }

        state
    }

    pub fn player_balance(&self, player_id: &str) -> i64 {
        self.balances
            .get(player_id)
            .map(PNCounter::value)
            .unwrap_or_default()
    }

    pub fn available_rights(&self, player_id: &str, replica_id: &str) -> i64 {
        self.spending_rights
            .get(player_id)
            .map(|counter| counter.available(replica_id))
            .unwrap_or_default()
    }

    pub fn round_phase(&self, round_id: &str) -> Option<RoundPhase> {
        self.round_views.get(round_id).map(|round| round.phase)
    }

    pub fn visible_bets(&self) -> BTreeSet<Bet> {
        self.bets.elements()
    }

    pub fn active_round_id(&self) -> Option<String> {
        self.round_views
            .values()
            .filter(|round| round.phase != RoundPhase::Resolved)
            .map(|round| round.id.clone())
            .max()
    }

    fn apply(&mut self, operation: &Operation) {
        match &operation.kind {
            OperationKind::CreateTable { name, .. } => {
                self.table_name.assign(
                    Some(name.clone()),
                    operation.lamport_time,
                    &operation.replica_id,
                );
            }
            OperationKind::JoinTable {
                player_id,
                display_name,
                starting_chips,
                initial_rights,
                ..
            } => {
                self.players.add(player_id.clone(), operation.op_id.clone());
                self.player_names
                    .entry(player_id.clone())
                    .and_modify(|register| {
                        register.assign(
                            display_name.clone(),
                            operation.lamport_time,
                            &operation.replica_id,
                        )
                    })
                    .or_insert_with(|| {
                        LwwRegister::new(
                            display_name.clone(),
                            operation.lamport_time,
                            &operation.replica_id,
                        )
                    });
                self.balances
                    .entry(player_id.clone())
                    .or_default()
                    .increment(&operation.replica_id, *starting_chips);

                let counter = self.spending_rights.entry(player_id.clone()).or_default();
                if initial_rights.is_empty() {
                    counter.set_initial_rights(&operation.replica_id, *starting_chips);
                } else {
                    for (replica_id, amount) in initial_rights {
                        counter.set_initial_rights(replica_id, *amount);
                    }
                }
            }
            OperationKind::LeaveTable { player_id, .. } => {
                self.players
                    .remove_observed(player_id, operation.op_id.clone());
            }
            OperationKind::CreateRound {
                round_id,
                dealer_id,
            } => {
                self.rounds.add(round_id.clone(), operation.op_id.clone());
                self.round_views
                    .entry(round_id.clone())
                    .or_insert_with(|| Round::new(round_id, dealer_id));
            }
            OperationKind::OpenBetting { round_id } => {
                let Some(round) = self.round_views.get_mut(round_id) else {
                    self.reject(operation, "round does not exist");
                    return;
                };
                if !round.open() {
                    self.reject(operation, "invalid round transition to BettingOpen");
                }
            }
            OperationKind::PlaceBet {
                bet_id,
                player_id,
                round_id,
                amount,
                bet_kind,
                origin_round_phase,
            } => {
                if !self.players.contains(player_id) {
                    self.reject(operation, "player has not joined");
                    return;
                }
                if !matches!(origin_round_phase, RoundPhase::BettingOpen) {
                    self.reject(operation, "origin replica did not have betting open");
                    return;
                }
                if !self.round_views.contains_key(round_id) {
                    self.reject(operation, "round does not exist");
                    return;
                }

                let rights = self.spending_rights.entry(player_id.clone()).or_default();
                if !rights.spend(operation.op_id.clone(), &operation.replica_id, *amount) {
                    self.reject(operation, "insufficient local spending rights");
                    return;
                }

                self.balances
                    .entry(player_id.clone())
                    .or_default()
                    .decrement(&operation.replica_id, *amount);
                self.bets.add(
                    Bet {
                        id: bet_id.clone(),
                        player_id: player_id.clone(),
                        round_id: round_id.clone(),
                        amount: *amount,
                        kind: bet_kind.clone(),
                        origin_replica: operation.replica_id.clone(),
                        origin_lamport: operation.lamport_time,
                    },
                    operation.op_id.clone(),
                );
            }
            OperationKind::CloseBetting { round_id } => {
                let Some(round) = self.round_views.get_mut(round_id) else {
                    self.reject(operation, "round does not exist");
                    return;
                };
                if !round.close() {
                    self.reject(operation, "invalid round transition to BettingClosed");
                }
            }
            OperationKind::ResolveRound { round_id, dice } => {
                let Some(round) = self.round_views.get_mut(round_id) else {
                    self.reject(operation, "round does not exist");
                    return;
                };
                if !round.resolve(*dice) {
                    self.reject(operation, "invalid round transition to Resolved");
                }
            }
            OperationKind::ApplyPayout {
                payout_id,
                player_id,
                amount,
                ..
            } => {
                if !self.applied_payouts.insert(payout_id.clone()) {
                    self.reject(operation, "duplicate payout id");
                    return;
                }
                self.balances
                    .entry(player_id.clone())
                    .or_default()
                    .increment(&operation.replica_id, *amount);
                self.spending_rights
                    .entry(player_id.clone())
                    .or_default()
                    .grant(payout_id, &operation.replica_id, *amount);
            }
            OperationKind::TransferSpendingRights {
                player_id,
                from_replica,
                to_replica,
                amount,
            } => {
                let counter = self.spending_rights.entry(player_id.clone()).or_default();
                if !counter.transfer(operation.op_id.clone(), from_replica, to_replica, *amount) {
                    self.reject(operation, "insufficient rights for transfer");
                }
            }
        }
    }

    fn reject(&mut self, operation: &Operation, reason: impl Into<String>) {
        self.rejected_operations
            .insert(operation.op_id.clone(), reason.into());
    }
}
