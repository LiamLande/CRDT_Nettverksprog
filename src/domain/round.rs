use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RoundPhase {
    Created,
    BettingOpen,
    BettingClosed,
    Resolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Round {
    pub id: String,
    pub dealer_id: String,
    pub phase: RoundPhase,
    pub result: Option<u8>,
}

impl Round {
    pub fn new(id: impl Into<String>, dealer_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            dealer_id: dealer_id.into(),
            phase: RoundPhase::Created,
            result: None,
        }
    }

    pub fn open(&mut self) -> bool {
        if self.phase != RoundPhase::Created {
            return false;
        }
        self.phase = RoundPhase::BettingOpen;
        true
    }

    pub fn close(&mut self) -> bool {
        if self.phase != RoundPhase::BettingOpen {
            return false;
        }
        self.phase = RoundPhase::BettingClosed;
        true
    }

    pub fn resolve(&mut self, dice: u8) -> bool {
        if self.phase != RoundPhase::BettingClosed || !(1..=6).contains(&dice) {
            return false;
        }
        self.phase = RoundPhase::Resolved;
        self.result = Some(dice);
        true
    }
}
