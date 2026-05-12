use crate::sync::Operation;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationLog {
    pub seen_operations: BTreeSet<String>,
    pub operations: Vec<Operation>,
}

impl OperationLog {
    pub fn insert(&mut self, operation: Operation) -> bool {
        if !self.seen_operations.insert(operation.op_id.clone()) {
            return false;
        }
        self.operations.push(operation);
        true
    }

    pub fn contains(&self, op_id: &str) -> bool {
        self.seen_operations.contains(op_id)
    }

    pub fn merge(&mut self, operations: impl IntoIterator<Item = Operation>) -> usize {
        operations
            .into_iter()
            .filter(|operation| self.insert(operation.clone()))
            .count()
    }

    pub fn sorted_operations(&self) -> Vec<Operation> {
        let mut operations = self.operations.clone();
        operations.sort_by(|left, right| {
            (left.lamport_time, &left.replica_id, &left.op_id).cmp(&(
                right.lamport_time,
                &right.replica_id,
                &right.op_id,
            ))
        });
        operations
    }

    pub fn ids(&self) -> Vec<String> {
        self.seen_operations.iter().cloned().collect()
    }
}
