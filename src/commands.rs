use crate::{resp::{AggregateValue, RespValue, SimpleValue}, store::Store};

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
    match args {
        [key, value] => {
            store.lock().unwrap().insert(key.clone(), value.clone());
            simple_string("OK")
        }
        _ => error("ERR wrong number of arguments for 'set' command"),
    }
}

fn get(args: &[Vec<u8>], store: &Store) -> RespValue {
    match args {
        [key] => {
            let value = store.lock().unwrap().get(key).cloned();
            match value {
                Some(bytes) => bulk_string(bytes),
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

    fn command(parts: &[&str]) -> RespValue {
        RespValue::Aggregate(AggregateValue::Array(
            parts
                .iter()
                .map(|p| RespValue::Aggregate(AggregateValue::BulkString(p.as_bytes().to_vec())))
                .collect(),
        ))
    }

    fn assert_error(reply: &RespValue) {
        assert!(
            matches!(reply, RespValue::Simple(SimpleValue::Error(_))),
            "expected an error reply, got {reply:?}"
        );
    }

    #[test]
    fn ping_without_args_replies_pong() {
        let store = new_store();
        assert_eq!(dispatch(&command(&["PING"]), &store), simple_string("PONG"));
    }

    #[test]
    fn ping_is_case_insensitive() {
        let store = new_store();
        assert_eq!(dispatch(&command(&["ping"]), &store), simple_string("PONG"));
    }

    #[test]
    fn ping_with_message_echoes_it_back() {
        let store = new_store();
        assert_eq!(
            dispatch(&command(&["PING", "hello"]), &store),
            bulk_string(b"hello".to_vec())
        );
    }

    #[test]
    fn ping_with_too_many_args_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&command(&["PING", "a", "b"]), &store));
    }

    #[test]
    fn echo_replies_with_the_message() {
        let store = new_store();
        assert_eq!(
            dispatch(&command(&["ECHO", "Hello World"]), &store),
            bulk_string(b"Hello World".to_vec())
        );
    }

    #[test]
    fn echo_without_args_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&command(&["ECHO"]), &store));
    }

    #[test]
    fn echo_with_too_many_args_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&command(&["ECHO", "a", "b"]), &store));
    }

    #[test]
    fn unknown_command_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&command(&["FLURB"]), &store));
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
            dispatch(&command(&["SET", "key", "value"]), &store),
            simple_string("OK")
        );
        assert_eq!(
            dispatch(&command(&["GET", "key"]), &store),
            bulk_string(b"value".to_vec())
        );
    }

    #[test]
    fn set_is_case_insensitive_like_every_other_command() {
        let store = new_store();
        assert_eq!(
            dispatch(&command(&["set", "key", "value"]), &store),
            simple_string("OK")
        );
    }

    #[test]
    fn set_overwrites_an_existing_value() {
        let store = new_store();
        dispatch(&command(&["SET", "key", "first"]), &store);
        dispatch(&command(&["SET", "key", "second"]), &store);
        assert_eq!(
            dispatch(&command(&["GET", "key"]), &store),
            bulk_string(b"second".to_vec())
        );
    }

    #[test]
    fn get_on_a_missing_key_returns_null() {
        let store = new_store();
        assert_eq!(
            dispatch(&command(&["GET", "missing"]), &store),
            RespValue::Simple(SimpleValue::Null)
        );
    }

    #[test]
    fn different_keys_do_not_collide() {
        let store = new_store();
        dispatch(&command(&["SET", "a", "1"]), &store);
        dispatch(&command(&["SET", "b", "2"]), &store);
        assert_eq!(
            dispatch(&command(&["GET", "a"]), &store),
            bulk_string(b"1".to_vec())
        );
        assert_eq!(
            dispatch(&command(&["GET", "b"]), &store),
            bulk_string(b"2".to_vec())
        );
    }

    #[test]
    fn set_with_no_args_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&command(&["SET"]), &store));
    }

    #[test]
    fn set_with_only_a_key_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&command(&["SET", "key"]), &store));
    }

    #[test]
    fn set_with_too_many_args_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(
            &command(&["SET", "key", "value", "extra"]),
            &store,
        ));
    }

    #[test]
    fn get_with_no_args_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&command(&["GET"]), &store));
    }

    #[test]
    fn get_with_too_many_args_is_an_error() {
        let store = new_store();
        assert_error(&dispatch(&command(&["GET", "a", "b"]), &store));
    }
}
