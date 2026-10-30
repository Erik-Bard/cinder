use crate::resp::RespValue;

use super::{bulk_string, error, simple_string};

pub fn ping(args: &[Vec<u8>]) -> RespValue {
    match args {
        [] => simple_string("PONG"),
        [message] => bulk_string(message.clone()),
        _ => error("ERR wrong number of arguments for 'ping' command"),
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::{assert_error, bulk_string, dispatch, simple_string};
    use crate::resp::RespValue;
    use crate::store::new_store;

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
}
