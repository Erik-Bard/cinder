use crate::{resp::{AggregateValue, RespValue, SimpleValue}, store::Store};

mod echo;
mod get;
mod ping;
mod set;

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
        "PING" => ping::ping(args),
        "ECHO" => echo::echo(args),
        "SET" => set::set(args, store),
        "GET" => get::get(args, store),
        other => error(&format!("ERR unknown command '{other}'")),
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
fn assert_error(reply: &RespValue) {
    assert!(
        matches!(reply, RespValue::Simple(SimpleValue::Error(_))),
        "expected an error reply, got {reply:?}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::new_store;

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
}
