use std::sync::{Mutex, OnceLock};

use serde_json::{Map, Value};

fn store() -> &'static Mutex<Map<String, Value>> {
    static STORE: OnceLock<Mutex<Map<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Map::new()))
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct GlobalState;

impl GlobalState {
    pub(crate) fn get(self, key: &str) -> Option<Value> {
        store().lock().unwrap().get(key).cloned()
    }

    pub(crate) fn set(self, key: String, value: Value) {
        store().lock().unwrap().insert(key, value);
    }

    pub(crate) fn delete(self, key: &str) -> bool {
        store().lock().unwrap().remove(key).is_some()
    }

    pub(crate) fn keys(self) -> Vec<String> {
        store().lock().unwrap().keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn set_then_get_round_trips_the_value() {
        let _guard = TEST_LOCK.lock().unwrap();
        GlobalState.set("count".to_owned(), Value::from(1));
        assert_eq!(GlobalState.get("count"), Some(Value::from(1)));
        GlobalState.delete("count");
    }

    #[test]
    fn missing_key_returns_none() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(GlobalState.get("missing-key"), None);
    }

    #[test]
    fn delete_removes_the_key() {
        let _guard = TEST_LOCK.lock().unwrap();
        GlobalState.set("temp".to_owned(), Value::from(true));
        assert!(GlobalState.delete("temp"));
        assert_eq!(GlobalState.get("temp"), None);
        assert!(!GlobalState.delete("temp"));
    }

    #[test]
    fn keys_lists_every_stored_key() {
        let _guard = TEST_LOCK.lock().unwrap();
        GlobalState.set("a".to_owned(), Value::from(1));
        GlobalState.set("b".to_owned(), Value::from(2));
        let mut keys = GlobalState.keys();
        keys.sort();
        assert_eq!(keys, vec!["a".to_owned(), "b".to_owned()]);
        GlobalState.delete("a");
        GlobalState.delete("b");
    }
}
