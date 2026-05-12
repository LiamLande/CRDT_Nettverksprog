use crate::sync::Operation;
use std::collections::BTreeSet;

#[derive(Debug, Default)]
pub struct OperationStore {
    seen: BTreeSet<String>,
    operations: Vec<Operation>,
}

impl OperationStore {
    pub fn insert(&mut self, operation: Operation) -> bool {
        if !self.seen.insert(operation.op_id.clone()) {
            return false;
        }
        self.operations.push(operation);
        true
    }

    pub fn missing_for(&self, seen_operations: &[String]) -> Vec<Operation> {
        let seen: BTreeSet<_> = seen_operations.iter().collect();
        self.operations
            .iter()
            .filter(|operation| !seen.contains(&operation.op_id))
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.operations.len()
    }
}
