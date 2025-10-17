use anyhow::{Result, bail};

#[derive(Debug, Default, Clone)]
pub struct Monitor {
    owner: Option<i64>,
    entry_count: u64,
}

impl Monitor {
    pub fn set_entry_count(&mut self, entry_count: u64) {
        self.entry_count = entry_count;
    }
    pub fn entry_count(&self) -> u64 {
        self.entry_count
    }

    pub fn increment_entry_count(&mut self) {
        self.entry_count += 1;
    }

    pub fn is_owner(&mut self, thread_id: i64) -> bool {
        if let Some(owner_tid) = self.owner {
            owner_tid == thread_id
        } else {
            false
        }
    }

    pub fn set_owner(&mut self, thread_id: i64) -> Result<()> {
        if let Some(tid) = self.owner {
            bail!("monitor is already owned by thread {tid}")
        }

        self.owner = Some(thread_id);
        Ok(())
    }
}
