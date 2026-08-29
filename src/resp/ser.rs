use super::value::{AggregateValue, RespValue, SimpleValue};

impl RespValue {
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        write_value(self, &mut out);
        out
    }
}

fn write_value(value: &RespValue, out: &mut Vec<u8>) {
    match value {
        RespValue::Simple(v) => write_simple(v, out),
        RespValue::Aggregate(v) => write_aggregate(v, out),
    }
}

fn write_line(out: &mut Vec<u8>, tag: u8, body: &[u8]) {
    out.push(tag);
    out.extend_from_slice(body);
    out.extend_from_slice(b"\r\n");
}

fn write_length_prefixed(out: &mut Vec<u8>, tag: u8, body: &[u8]) {
    out.push(tag);
    out.extend_from_slice(body.len().to_string().as_bytes());
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(body);
    out.extend_from_slice(b"\r\n");
}

fn write_collection<'a>(
    out: &mut Vec<u8>,
    tag: u8,
    items: impl ExactSizeIterator<Item = &'a RespValue>,
) {
    out.push(tag);
    out.extend_from_slice(items.len().to_string().as_bytes());
    out.extend_from_slice(b"\r\n");
    for item in items {
        write_value(item, out);
    }
}

fn format_double(d: f64) -> String {
    if d.is_nan() {
        "nan".to_string()
    } else if d.is_infinite() {
        if d.is_sign_negative() {
            "-inf".to_string()
        } else {
            "inf".to_string()
        }
    } else {
        d.to_string()
    }
}

fn write_simple(value: &SimpleValue, out: &mut Vec<u8>) {
    match value {
        SimpleValue::SimpleString(bytes) => write_line(out, b'+', bytes),
        SimpleValue::Error(bytes) => write_line(out, b'-', bytes),
        SimpleValue::Integer(n) => write_line(out, b':', n.to_string().as_bytes()),
        SimpleValue::Null => write_line(out, b'_', b""),
        SimpleValue::Boolean(b) => write_line(out, b'#', if *b { b"t" } else { b"f" }),
        SimpleValue::Double(d) => write_line(out, b',', format_double(*d).as_bytes()),
        SimpleValue::BigNumber(digits) => write_line(out, b'(', digits),
    }
}

fn write_aggregate(value: &AggregateValue, out: &mut Vec<u8>) {
    match value {
        AggregateValue::BulkString(bytes) => write_length_prefixed(out, b'$', bytes),
        AggregateValue::BulkError(bytes) => write_length_prefixed(out, b'!', bytes),
        AggregateValue::VerbatimString { encoding, text } => {
            let mut body = Vec::with_capacity(4 + text.len());
            body.extend_from_slice(encoding);
            body.push(b':');
            body.extend_from_slice(text);
            write_length_prefixed(out, b'=', &body);
        }
        AggregateValue::Array(items) => write_collection(out, b'*', items.iter()),
        AggregateValue::Set(items) => write_collection(out, b'~', items.iter()),
        AggregateValue::Push(items) => write_collection(out, b'>', items.iter()),
        AggregateValue::Map(pairs) => {
            out.push(b'%');
            out.extend_from_slice(pairs.len().to_string().as_bytes());
            out.extend_from_slice(b"\r\n");
            for (k, v) in pairs {
                write_value(k, out);
                write_value(v, out);
            }
        }
        AggregateValue::Attribute { attributes, value } => {
            out.push(b'|');
            out.extend_from_slice(attributes.len().to_string().as_bytes());
            out.extend_from_slice(b"\r\n");
            for (k, v) in attributes {
                write_value(k, out);
                write_value(v, out);
            }
            write_value(value, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn assert_round_trips(input: &[u8]) {
        let value = RespValue::deserialize(input).expect("expected input to deserialize");
        assert_eq!(value.serialize(), input);
    }

    #[test]
    fn simple_string_round_trips() {
        assert_round_trips(b"+OK\r\n");
    }

    #[test]
    fn ping_command_round_trips() {
        assert_round_trips(b"*1\r\n$4\r\nping\r\n");
    }

    #[test]
    fn map_round_trips() {
        assert_round_trips(b"%1\r\n+key\r\n:1\r\n");
    }

    #[test]
    fn canonical_null_round_trips() {
        assert_round_trips(b"_\r\n");
    }
}
