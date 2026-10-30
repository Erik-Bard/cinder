use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{resp::{AggregateValue, RespValue, SimpleValue}, store::{Entry, Store}};

pub fn dispatch(request: &RespValue, store: &Store) -> RespValue {
    match parse_command(request) {
        Ok((name, args)) => run(&name, &args, &store),
        Err(message) => error(&message),
    }
}

fn parse_command(request: &RespValue) -> Result<(String, Vec<Vec<u8>>), String> {
    let RespValue::Aggregate(AggregateValue::Array(items)) = request else {
        return Err("ERR expected a RESP array of bulk strings".to_string());
    };

    let mut parts = Vec::with_capacity(items.len());
    for item in items {
        let RespValue::Aggregate(AggregateValue::BulkString(bytes)) = item else {
            return Err("ERR expected bulk strings as array elements".to_string());
        };
        parts.push(bytes.clone());
    }

    let Some((name, args)) = parts.split_first() else {
        return Err("ERR empty command".to_string());
    };

    Ok((String::from_utf8_lossy(name).to_ascii_uppercase(), args.to_vec()))
}

fn run(name: &str, args: &[Vec<u8>], store: &Store) -> RespValue {
    match name {
        "PING" => ping(args),
        "ECHO" => echo(args),
        "SET" => set(args, store),
        "GET" => get(args, store),
        other => error(&format!("ERR unknown command '{other}'")),
    }
}

fn ping(args: &[Vec<u8>]) -> RespValue {
    match args {
        [] => simple_string("PONG"),
        [message] => bulk_string(message.clone()),
        _ => error("ERR wrong number of arguments for 'ping' command"),
    }
}

fn echo(args: &[Vec<u8>]) -> RespValue {
    match args {
        [message] => bulk_string(message.clone()),
        _ => error("ERR wrong number of arguments for 'echo' command"),
    }
}

fn set(args: &[Vec<u8>], store: &Store) -> RespValue {
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

fn get(args: &[Vec<u8>], store: &Store) -> RespValue {
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

fn simple_string(s: &str) -> RespValue {
    RespValue::Simple(SimpleValue::SimpleString(s.as_bytes().to_vec()))
}

fn bulk_string(bytes: Vec<u8>) -> RespValue {
    RespValue::Aggregate(AggregateValue::BulkString(bytes))
}

fn error(message: &str) -> RespValue {
    RespValue::Simple(SimpleValue::Error(message.as_bytes().to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::new_store;

    fn assert_error(reply: &RespValue) {
        assert!(
            matches!(reply, RespValue::Simple(SimpleValue::Error(_))),
            "expected an error reply, got {reply:?}"
        );
    }

    #[test]
    fn ping_without_args_replies_pong() {
        let store = new_store();
        assert_eq!(dispatch(&RespValue::command(&["PING"]), &store), simple_string("PONG"));
    }

    #[test]
    fn ping_is_case_insensitive() {
        let store = new_store();
        assert_eq!(dispatch(&RespValue::command(&["ping"]), &store), simple_string("PONG"));
    }

    #[test]
    fn ping_with_message_echoes_it_back() {
        let store = new_store();
        assert_eq!(
            dispatch(&RespValue::command(&["PING", "hello"]), &store),
            bulk_string(b"hello".to_vec())
        );
    }

    #[test]
    fn ping_with_too_many_args_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&RespValue::command(&["PING", "a", "b"]), &store));
    }

    #[test]
    fn echo_replies_with_the_message() {
        let store = new_store();
        assert_eq!(
            dispatch(&RespValue::command(&["ECHO", "Hello World"]), &store),
            bulk_string(b"Hello World".to_vec())
        );
    }

    #[test]
    fn echo_without_args_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&RespValue::command(&["ECHO"]), &store));
    }

    #[test]
    fn echo_with_too_many_args_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&RespValue::command(&["ECHO", "a", "b"]), &store));
    }

    #[test]
    fn unknown_command_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&RespValue::command(&["FLURB"]), &store));
    }

    #[test]
    fn non_array_request_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(
            &RespValue::Simple(SimpleValue::SimpleString(b"PING".to_vec())),
            &store,
        ));
    }

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
    fn get_on_a_missing_key_returns_null() {
        let store = new_store();
        assert_eq!(
            dispatch(&RespValue::command(&["GET", "missing"]), &store),
            RespValue::Simple(SimpleValue::Null)
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
