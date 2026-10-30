use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{resp::RespValue, store::{Entry, Store}};

use super::{error, simple_string};

pub fn set(args: &[Vec<u8>], store: &Store) -> RespValue {
    let [key, value, options @ ..] = args else {
        return error("ERR wrong number of arguments for 'set' command");
    };
    let expires_at = match parse_expiry(options) {
        Ok(expires_at) => expires_at,
          Err(message) => return error(message),
    };
    store.lock().unwrap().insert(key.clone(), Entry { value: value.clone(), expires_at });
    simple_string("OK")
}

fn parse_expiry(options: &[Vec<u8>]) -> Result<Option<SystemTime>, &'static str> {
    const INVALID: &str = "ERR invalid expire time found in 'SET' command";
    match options {
        [] => Ok(None),
        [flag, amount] => {
            let n: u64 = std::str::from_utf8(amount).ok()
                .and_then(|s| s.parse().ok())
                .filter(|&n| n > 0)
                .ok_or(INVALID)?;
            let (base, offset) = match flag.to_ascii_uppercase().as_slice() {
                b"EX" => (SystemTime::now(), Duration::from_secs(n)),
                  b"PX" => (SystemTime::now(), Duration::from_millis(n)),
                  b"EXAT" => (UNIX_EPOCH, Duration::from_secs(n)),
                  b"PXAT" => (UNIX_EPOCH, Duration::from_millis(n)),
                  _ => return Err("ERR syntax error")
            };
            base.checked_add(offset).map(Some).ok_or(INVALID)
        }
        _ => Err("ERR syntax error"),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::commands::{assert_error, bulk_string, dispatch, simple_string};
    use crate::resp::{RespValue, SimpleValue};
    use crate::store::new_store;

    #[test]
    fn set_then_get_returns_the_stored_value() {
        let store = new_store();
        assert_eq!(
            dispatch(&RespValue::command(&["SET", "key", "value"]), &store),
            simple_string("OK")
        );
        assert_eq!(
            dispatch(&RespValue::command(&["GET", "key"]), &store),
            bulk_string(b"value".to_vec())
        );
    }

    #[test]
    fn set_is_case_insensitive_like_every_other_command() {
        let store = new_store();
        assert_eq!(
            dispatch(&RespValue::command(&["set", "key", "value"]), &store),
            simple_string("OK")
        );
    }

    #[test]
    fn set_overwrites_an_existing_value() {
        let store = new_store();
        dispatch(&RespValue::command(&["SET", "key", "first"]), &store);
        dispatch(&RespValue::command(&["SET", "key", "second"]), &store);
        assert_eq!(
            dispatch(&RespValue::command(&["GET", "key"]), &store),
            bulk_string(b"second".to_vec())
        );
    }

    #[test]
    fn different_keys_do_not_collide() {
        let store = new_store();
        dispatch(&RespValue::command(&["SET", "a", "1"]), &store);
        dispatch(&RespValue::command(&["SET", "b", "2"]), &store);
        assert_eq!(
            dispatch(&RespValue::command(&["GET", "a"]), &store),
            bulk_string(b"1".to_vec())
        );
        assert_eq!(
            dispatch(&RespValue::command(&["GET", "b"]), &store),
            bulk_string(b"2".to_vec())
        );
    }

    #[test]
    fn set_with_no_args_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&RespValue::command(&["SET"]), &store));
    }

    #[test]
    fn set_with_only_a_key_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&RespValue::command(&["SET", "key"]), &store));
    }

    #[test]
    fn set_with_too_many_args_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(
            &RespValue::command(&["SET", "key", "value", "extra"]),
            &store,
        ));
    }

    #[test]
    fn set_with_each_future_expiry_option_keeps_the_value() {
        let store = new_store();
        let far_future_secs = "4102444800"; // 2100-01-01
        let far_future_ms = "4102444800000";
        for (flag, amount) in [("EX", "100"), ("PX", "100000"), ("EXAT", far_future_secs), ("PXAT", far_future_ms)] {
            assert_eq!(
                dispatch(&RespValue::command(&["SET", "key", "value", flag, amount]), &store),
                simple_string("OK"),
                "SET with {flag} {amount}"
            );
            assert_eq!(
                dispatch(&RespValue::command(&["GET", "key"]), &store),
                bulk_string(b"value".to_vec()),
                "GET after SET with {flag} {amount}"
            );
        }
    }

    #[test]
    fn set_without_options_clears_a_previous_expiry() {
        let store = new_store();
        dispatch(&RespValue::command(&["SET", "key", "first", "PX", "50"]), &store);
        dispatch(&RespValue::command(&["SET", "key", "second"]), &store);
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            dispatch(&RespValue::command(&["GET", "key"]), &store),
            bulk_string(b"second".to_vec())
        );
    }

    #[test]
    fn expiry_flags_are_case_insensitive() {
        let store = new_store();
        assert_eq!(
            dispatch(&RespValue::command(&["SET", "key", "value", "ex", "100"]), &store),
            simple_string("OK")
        );
    }

    #[test]
    fn set_with_an_invalid_expire_time_is_an_error() {
        let store = new_store();
        for amount in ["0", "-5", "abc", "", "1.5", "18446744073709551616"] {
            assert_error(&dispatch(
                &RespValue::command(&["SET", "key", "value", "EX", amount]),
                &store,
            ));
        }
    }

    #[test]
    fn set_with_an_expire_time_that_overflows_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(
            &RespValue::command(&["SET", "key", "value", "EXAT", "18446744073709551615"]),
            &store,
        ));
    }

    #[test]
    fn set_with_an_expiry_flag_but_no_amount_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&RespValue::command(&["SET", "key", "value", "EX"]), &store));
    }

    #[test]
    fn set_with_an_unknown_option_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(
            &RespValue::command(&["SET", "key", "value", "FOO", "10"]),
            &store,
        ));
    }

    #[test]
    fn set_with_two_expiry_options_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(
            &RespValue::command(&["SET", "key", "value", "EX", "10", "PX", "100"]),
            &store,
        ));
    }

    #[test]
    fn a_rejected_set_does_not_store_the_value() {
        let store = new_store();
        dispatch(&RespValue::command(&["SET", "key", "value", "EX", "0"]), &store);
        assert_eq!(
            dispatch(&RespValue::command(&["GET", "key"]), &store),
            RespValue::Simple(SimpleValue::Null)
        );
    }
}
