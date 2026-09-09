use crate::resp::{AggregateValue, RespValue, SimpleValue};

pub fn dispatch(request: &RespValue) -> RespValue {
    match parse_command(request) {
        Ok((name, args)) => run(&name, &args),
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

fn run(name: &str, args: &[Vec<u8>]) -> RespValue {
    match name {
        "PING" => ping(args),
        "ECHO" => echo(args),
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
        assert_eq!(dispatch(&command(&["PING"])), simple_string("PONG"));
    }

    #[test]
    fn ping_is_case_insensitive() {
        assert_eq!(dispatch(&command(&["ping"])), simple_string("PONG"));
    }

    #[test]
    fn ping_with_message_echoes_it_back() {
        assert_eq!(
            dispatch(&command(&["PING", "hello"])),
            bulk_string(b"hello".to_vec())
        );
    }

    #[test]
    fn ping_with_too_many_args_is_an_error() {
        assert_error(&dispatch(&command(&["PING", "a", "b"])));
    }

    #[test]
    fn echo_replies_with_the_message() {
        assert_eq!(
            dispatch(&command(&["ECHO", "Hello World"])),
            bulk_string(b"Hello World".to_vec())
        );
    }

    #[test]
    fn echo_without_args_is_an_error() {
        assert_error(&dispatch(&command(&["ECHO"])));
    }

    #[test]
    fn echo_with_too_many_args_is_an_error() {
        assert_error(&dispatch(&command(&["ECHO", "a", "b"])));
    }

    #[test]
    fn unknown_command_is_an_error() {
        assert_error(&dispatch(&command(&["FLURB"])));
    }

    #[test]
    fn non_array_request_is_an_error() {
        assert_error(&dispatch(&RespValue::Simple(SimpleValue::SimpleString(
            b"PING".to_vec(),
        ))));
    }
}
