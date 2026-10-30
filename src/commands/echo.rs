use crate::resp::RespValue;

use super::{bulk_string, error};

pub fn echo(args: &[Vec<u8>]) -> RespValue {
    match args {
        [message] => bulk_string(message.clone()),
        _ => error("ERR wrong number of arguments for 'echo' command"),
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::{assert_error, bulk_string, dispatch};
    use crate::resp::RespValue;
    use crate::store::new_store;

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
}
