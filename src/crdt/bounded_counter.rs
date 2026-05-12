use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SpendEntry {
    pub replica_id: String,
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GrantEntry {
    pub replica_id: String,
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TransferEntry {
    pub from_replica: String,
    pub to_replica: String,
    pub amount: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedCounter {
    pub initial_rights: BTreeMap<String, u64>,
    pub grants: BTreeMap<String, GrantEntry>,
    pub spends: BTreeMap<String, SpendEntry>,
    pub transfers: BTreeMap<String, TransferEntry>,
}

impl BoundedCounter {
    pub fn set_initial_rights(&mut self, replica_id: impl Into<String>, amount: u64) {
        let entry = self.initial_rights.entry(replica_id.into()).or_default();
        *entry = (*entry).max(amount);
    }

    pub fn grant(&mut self, op_id: impl Into<String>, replica_id: impl Into<String>, amount: u64) {
        self.grants.insert(
            op_id.into(),
            GrantEntry {
                replica_id: replica_id.into(),
                amount,
            },
        );
    }

    pub fn spend(
        &mut self,
        op_id: impl Into<String>,
        replica_id: impl Into<String>,
        amount: u64,
    ) -> bool {
        let op_id = op_id.into();
        if self.spends.contains_key(&op_id) {
            return true;
        }

        let replica_id = replica_id.into();
        if self.available(&replica_id) < amount as i64 {
            return false;
        }

        self.spends.insert(op_id, SpendEntry { replica_id, amount });
        true
    }

    pub fn transfer(
        &mut self,
        op_id: impl Into<String>,
        from_replica: impl Into<String>,
        to_replica: impl Into<String>,
        amount: u64,
    ) -> bool {
        let op_id = op_id.into();
        if self.transfers.contains_key(&op_id) {
            return true;
        }

        let from_replica = from_replica.into();
        if self.available(&from_replica) < amount as i64 {
            return false;
        }

        self.transfers.insert(
            op_id,
            TransferEntry {
                from_replica,
                to_replica: to_replica.into(),
                amount,
            },
        );
        true
    }

    pub fn available(&self, replica_id: &str) -> i64 {
        let initial = self.initial_rights.get(replica_id).copied().unwrap_or(0) as i64;
        let grants = self
            .grants
            .values()
            .filter(|grant| grant.replica_id == replica_id)
            .map(|grant| grant.amount as i64)
            .sum::<i64>();
        let incoming = self
            .transfers
            .values()
            .filter(|transfer| transfer.to_replica == replica_id)
            .map(|transfer| transfer.amount as i64)
            .sum::<i64>();
        let outgoing = self
            .transfers
            .values()
            .filter(|transfer| transfer.from_replica == replica_id)
            .map(|transfer| transfer.amount as i64)
            .sum::<i64>();
        let spent = self
            .spends
            .values()
            .filter(|spend| spend.replica_id == replica_id)
            .map(|spend| spend.amount as i64)
            .sum::<i64>();

        initial + grants + incoming - outgoing - spent
    }

    pub fn total_remaining(&self) -> i64 {
        let mut replicas: Vec<_> = self.initial_rights.keys().cloned().collect();
        replicas.extend(self.grants.values().map(|grant| grant.replica_id.clone()));
        replicas.extend(
            self.transfers
                .values()
                .flat_map(|transfer| [transfer.from_replica.clone(), transfer.to_replica.clone()]),
        );
        replicas.sort();
        replicas.dedup();
        replicas
            .iter()
            .map(|replica_id| self.available(replica_id))
            .sum()
    }

    pub fn total_spent(&self) -> u64 {
        self.spends.values().map(|spend| spend.amount).sum()
    }

    pub fn merge(&self, other: &Self) -> Self {
        let mut merged = self.clone();
        merge_max_map(&mut merged.initial_rights, &other.initial_rights);
        merge_entry_map(&mut merged.grants, &other.grants);
        merge_entry_map(&mut merged.spends, &other.spends);
        merge_entry_map(&mut merged.transfers, &other.transfers);
        merged
    }

    pub fn merge_mut(&mut self, other: &Self) {
        *self = self.merge(other);
    }
}

fn merge_max_map(target: &mut BTreeMap<String, u64>, source: &BTreeMap<String, u64>) {
    for (key, value) in source {
        let entry = target.entry(key.clone()).or_default();
        *entry = (*entry).max(*value);
    }
}

fn merge_entry_map<T: Ord + Clone>(target: &mut BTreeMap<String, T>, source: &BTreeMap<String, T>) {
    for (key, value) in source {
        match target.get(key) {
            Some(existing) if existing >= value => {}
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}
