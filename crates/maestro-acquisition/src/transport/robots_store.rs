//! Bounded robots entries; different policy digests on one origin coexist.
use super::robots::{RobotsBinding, RobotsCache};
use crate::policy::identity::FetchIdentity;
use std::{collections::VecDeque, num::NonZeroUsize};

/// FIFO eviction over exact (origin, agent, checked-policy digest) keys.
#[derive(Debug)]
pub struct RobotsStore {
    /// Explicit host capacity, with no implicit unbounded default.
    capacity: NonZeroUsize,
    /// Owned entries, oldest first; N10 still checks each entry's TTL.
    entries: VecDeque<RobotsCache>,
}
impl RobotsStore {
    /// Bind a finite nonzero entry ceiling supplied by host settings.
    #[must_use]
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity,
            entries: VecDeque::new(),
        }
    }
    /// Retain a fetched result; replace only the same full tuple.
    pub fn insert(&mut self, entry: RobotsCache) {
        self.entries.retain(|old| !old.same_key(&entry));
        if self.entries.len() >= self.capacity.get() {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }
    /// A digest/agent/origin change is a miss, never stale reuse.
    #[must_use]
    pub fn get(
        &self,
        identity: &FetchIdentity,
        binding: RobotsBinding<'_>,
    ) -> Option<&RobotsCache> {
        self.entries
            .iter()
            .find(|entry| entry.matches(identity, binding))
    }
    /// Current bounded number of retained tuples.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// Whether the caller must perform its first admitted fetch.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
