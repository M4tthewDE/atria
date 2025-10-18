use std::collections::HashMap;

use anyhow::{Result, bail};
use tracing::info;

use crate::{heap::HeapId, thread::ThreadId};

#[derive(Debug)]
pub struct Monitor {
    entry_count: u64,
    owner: Option<ThreadId>,
}

impl Monitor {
    fn new(thread_id: ThreadId) -> Self {
        Self {
            entry_count: 1,
            owner: Some(thread_id),
        }
    }

    fn owned_by(&self, thread_id: &ThreadId) -> bool {
        if let Some(owner) = &self.owner {
            owner == thread_id
        } else {
            false
        }
    }
}

#[derive(Debug, Default)]
pub struct Monitors {
    object_monitors: HashMap<HeapId, Monitor>,
}

impl Monitors {
    pub fn enter_object_monitor(&mut self, heap_id: &HeapId, thread_id: &ThreadId) -> bool {
        if let Some(monitor) = self.object_monitors.get_mut(heap_id) {
            if monitor.owned_by(thread_id) {
                monitor.entry_count += 1;
            } else {
                return false;
            }
        } else {
            let monitor = Monitor::new(thread_id.clone());
            self.object_monitors.insert(heap_id.clone(), monitor);
        }

        info!("entered monitor for {heap_id:?} with thread {thread_id:?}");

        true
    }

    pub fn exit_object_monitor(&mut self, heap_id: &HeapId, thread_id: &ThreadId) -> Result<()> {
        if let Some(monitor) = self.object_monitors.get_mut(heap_id) {
            if monitor.owned_by(thread_id) {
                monitor.entry_count -= 1;
                if monitor.entry_count == 0 {
                    monitor.owner = None;
                    info!("thread {thread_id:?} is no longer the owner of {heap_id:?}");
                }
                info!("exited monitor for {heap_id:?} with thread {thread_id:?}");
                Ok(())
            } else {
                bail!("TODO: IllegalMonitorAccessException");
            }
        } else {
            bail!("no monitor found for {heap_id:?}");
        }
    }
}
