#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LamportClock {
    time: u64,
}

impl LamportClock {
    pub fn new() -> Self {
        Self { time: 0 }
    }

    pub fn tick(&mut self) -> u64 {
        self.time += 1;
        self.time
    }

    pub fn observe(&mut self, remote_time: u64) {
        self.time = self.time.max(remote_time);
    }

    pub fn now(&self) -> u64 {
        self.time
    }
}

impl Default for LamportClock {
    fn default() -> Self {
        Self::new()
    }
}
