#[derive(Debug, Default, Clone)]
pub struct Monitor {
    entry_count: u64,
}

impl Monitor {
    pub fn entry_count(&self) -> u64 {
        self.entry_count
    }
}
