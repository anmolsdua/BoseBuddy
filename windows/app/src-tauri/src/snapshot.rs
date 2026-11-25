//! The state the flyout renders, numbered. The worker publishes every change
//! as a new revision; the flyout subscribes first, then asks for the latest
//! snapshot, and ignores anything older than what it shows. So an update that
//! arrives while the flyout is starting is never lost, and a stale snapshot
//! never overwrites a newer event.

use std::sync::Mutex;

use qcbuddy_core::State;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub revision: u64,
    pub state: State,
}

#[derive(Default)]
pub struct Latest {
    snapshot: Mutex<Option<Snapshot>>,
}

impl Latest {
    /// Store `state` as the next revision and return it, ready to emit.
    /// Stored before it's emitted, so `get` never returns something older
    /// than an event the flyout already has.
    pub fn record(&self, state: &State) -> Snapshot {
        let mut slot = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        let revision = slot.as_ref().map_or(1, |s| s.revision + 1);
        let snapshot = Snapshot { revision, state: state.clone() };
        *slot = Some(snapshot.clone());
        snapshot
    }

    pub fn get(&self) -> Option<Snapshot> {
        self.snapshot.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revisions_increase_and_get_returns_the_newest() {
        let latest = Latest::default();
        assert!(latest.get().is_none(), "nothing before the first publish");
        let mut state = State::default();
        assert_eq!(latest.record(&state).revision, 1);
        state.battery = Some(55);
        let second = latest.record(&state);
        assert_eq!(second.revision, 2);
        let got = latest.get().unwrap();
        assert_eq!((got.revision, got.state.battery), (2, Some(55)));
    }

    #[test]
    fn serializes_for_the_flyout() {
        let json = serde_json::to_value(Latest::default().record(&State::default())).unwrap();
        assert_eq!(json["revision"], 1);
        assert_eq!(json["state"]["link"]["kind"], "disconnected");
    }
}
