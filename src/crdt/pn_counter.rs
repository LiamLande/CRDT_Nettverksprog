use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PNCounter {
    pub increments: BTreeMap<String, u64>,
    pub decrements: BTreeMap<String, u64>,
}

impl PNCounter {
    pub fn increment(&mut self, replica_id: impl Into<String>, amount: u64) {
        *self.increments.entry(replica_id.into()).or_default() += amount;
    }

    pub fn decrement(&mut self, replica_id: impl Into<String>, amount: u64) {
        *self.decrements.entry(replica_id.into()).or_default() += amount;
    }

    pub fn value(&self) -> i64 {
        let increments: u64 = self.increments.values().sum();
        let decrements: u64 = self.decrements.values().sum();
        increments as i64 - decrements as i64
    }

    pub fn merge(&self, other: &Self) -> Self {
        let mut merged = self.clone();
        merge_max_map(&mut merged.increments, &other.increments);
        merge_max_map(&mut merged.decrements, &other.decrements);
        merged
    }

    pub fn merge_mut(&mut self, other: &Self) {
        *self = self.merge(other);
    }
}

fn merge_max_map(target: &mut BTreeMap<String, u64>, source: &BTreeMap<String, u64>) {
    for (replica_id, value) in source {
        let entry = target.entry(replica_id.clone()).or_default();
        *entry = (*entry).max(*value);
    }
}
