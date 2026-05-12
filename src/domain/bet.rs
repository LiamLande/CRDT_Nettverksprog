use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BetKind {
    Odd,
    Even,
    High,
    Low,
    Exact(u8),
}

impl BetKind {
    pub fn wins(&self, dice: u8) -> bool {
        match self {
            BetKind::Odd => dice % 2 == 1,
            BetKind::Even => dice % 2 == 0,
            BetKind::High => (4..=6).contains(&dice),
            BetKind::Low => (1..=3).contains(&dice),
            BetKind::Exact(expected) => *expected == dice,
        }
    }

    pub fn multiplier(&self) -> u64 {
        match self {
            BetKind::Exact(_) => 6,
            BetKind::Odd | BetKind::Even | BetKind::High | BetKind::Low => 2,
        }
    }
}

impl fmt::Display for BetKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BetKind::Odd => write!(formatter, "odd"),
            BetKind::Even => write!(formatter, "even"),
            BetKind::High => write!(formatter, "high"),
            BetKind::Low => write!(formatter, "low"),
            BetKind::Exact(value) => write!(formatter, "exact:{value}"),
        }
    }
}

impl FromStr for BetKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "odd" => Ok(BetKind::Odd),
            "even" => Ok(BetKind::Even),
            "high" => Ok(BetKind::High),
            "low" => Ok(BetKind::Low),
            value if value.starts_with("exact:") => {
                let number = value
                    .trim_start_matches("exact:")
                    .parse::<u8>()
                    .map_err(|_| "exact bet must be exact:1 through exact:6".to_string())?;
                if (1..=6).contains(&number) {
                    Ok(BetKind::Exact(number))
                } else {
                    Err("exact bet must be exact:1 through exact:6".to_string())
                }
            }
            value if value.starts_with("exact") => {
                let number = value
                    .trim_start_matches("exact")
                    .parse::<u8>()
                    .map_err(|_| "exact bet must be exact1 through exact6".to_string())?;
                if (1..=6).contains(&number) {
                    Ok(BetKind::Exact(number))
                } else {
                    Err("exact bet must be exact1 through exact6".to_string())
                }
            }
            _ => Err("bet kind must be odd, even, high, low, exact:N, or exactN".to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Bet {
    pub id: String,
    pub player_id: String,
    pub round_id: String,
    pub amount: u64,
    pub kind: BetKind,
    pub origin_replica: String,
    pub origin_lamport: u64,
}
