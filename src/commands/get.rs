use std::time::SystemTime;

use crate::{resp::{RespValue, SimpleValue}, store::Store};

use super::{bulk_string, error};

pub fn get(args: &[Vec<u8>], store: &Store) -> RespValue {
    match args {
        [key] => {
            let mut map = store.lock().unwrap();
            if map.get(key).is_some_and(|e| e.expires_at.is_some_and(|t| t <= SystemTime::now())) {
                map.remove(key);
            }
            match map.get(key) {
                Some(entry) => bulk_string(entry.value.clone()),
                None => RespValue::Simple(SimpleValue::Null),
            }
        },
        _ => error("ERR wrong number of arguments for 'get' command"),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::commands::{assert_error, bulk_string, dispatch};
    use crate::resp::{RespValue, SimpleValue};
    use crate::store::new_store;

    #[test]
    fn get_on_a_missing_key_returns_null() {
        let store = new_store();
        assert_eq!(
            dispatch(&RespValue::command(&["GET", "missing"]), &store),
            RespValue::Simple(SimpleValue::Null)
        );
    }

    #[test]
    fn get_with_no_args_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&RespValue::command(&["GET"]), &store));
    }

    #[test]
    fn get_with_too_many_args_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&RespValue::command(&["GET", "a", "b"]), &store));
    }

    #[test]
    fn get_after_a_past_absolute_expiry_returns_null() {
        let store = new_store();
        for (flag, amount) in [("EXAT", "1"), ("PXAT", "1")] {
            dispatch(&RespValue::command(&["SET", "key", "value", flag, amount]), &store);
            assert_eq!(
                dispatch(&RespValue::command(&["GET", "key"]), &store),
                RespValue::Simple(SimpleValue::Null),
                "GET after SET with {flag} {amount}"
            );
        }
    }

    #[test]
    fn get_returns_the_value_before_a_relative_expiry_and_null_after() {
        let store = new_store();
        dispatch(&RespValue::command(&["SET", "key", "value", "PX", "50"]), &store);
        assert_eq!(
            dispatch(&RespValue::command(&["GET", "key"]), &store),
            bulk_string(b"value".to_vec())
        );
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            dispatch(&RespValue::command(&["GET", "key"]), &store),
            RespValue::Simple(SimpleValue::Null)
        );
    }

    #[test]
    fn get_removes_an_expired_key_from_the_store() {
        let store = new_store();
        dispatch(&RespValue::command(&["SET", "key", "value", "PXAT", "1"]), &store);
        dispatch(&RespValue::command(&["GET", "key"]), &store);
        assert!(!store.lock().unwrap().contains_key(b"key".as_slice()));
    }
}
