use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LwwRegister<T> {
    pub value: T,
    pub timestamp: u64,
    pub replica_id: String,
}

impl<T: Clone> LwwRegister<T> {
    pub fn new(value: T, timestamp: u64, replica_id: impl Into<String>) -> Self {
        Self {
            value,
            timestamp,
            replica_id: replica_id.into(),
        }
    }

    pub fn assign(&mut self, value: T, timestamp: u64, replica_id: impl Into<String>) {
        let candidate = Self::new(value, timestamp, replica_id);
        if candidate.wins_over(self) {
            *self = candidate;
        }
    }

    pub fn merge(&self, other: &Self) -> Self {
        if other.wins_over(self) {
            other.clone()
        } else {
            self.clone()
        }
    }

    pub fn merge_mut(&mut self, other: &Self) {
        *self = self.merge(other);
    }

    fn wins_over(&self, other: &Self) -> bool {
        self.timestamp > other.timestamp
            || (self.timestamp == other.timestamp && self.replica_id > other.replica_id)
    }
}
