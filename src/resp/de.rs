use super::error::RespError;
use super::tag::{AggregateType, RespType, SimpleType};
use super::value::{AggregateValue, RespValue, SimpleValue};

impl RespValue {
    pub fn deserialize(input: &[u8]) -> Result<RespValue, RespError> {
        let (value, rest) = parse_value(input)?;
        if !rest.is_empty() {
            return Err(RespError::TrailingData);
        }
        Ok(value)
    }
}

enum Length {
    Known(usize),
    Null,
}

fn parse_length(bytes: &[u8], allow_legacy_null: bool) -> Result<Length, RespError> {
    let text = std::str::from_utf8(bytes).map_err(|_| RespError::InvalidLength(bytes.to_vec()))?;
    let n: i64 = text
        .parse()
        .map_err(|_| RespError::InvalidLength(bytes.to_vec()))?;

    if n == -1 && allow_legacy_null {
        Ok(Length::Null)
    } else if n < 0 {
        Err(RespError::InvalidLength(bytes.to_vec()))
    } else {
        Ok(Length::Known(n as usize))
    }
}

fn parse_i64(bytes: &[u8]) -> Result<i64, RespError> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| RespError::InvalidInteger(bytes.to_vec()))
}

fn parse_f64(bytes: &[u8]) -> Result<f64, RespError> {
    match bytes {
        b"inf" => Ok(f64::INFINITY),
        b"-inf" => Ok(f64::NEG_INFINITY),
        b"nan" => Ok(f64::NAN),
        _ => std::str::from_utf8(bytes)
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| RespError::InvalidDouble(bytes.to_vec())),
    }
}

fn is_valid_big_number(bytes: &[u8]) -> bool {
    let digits = bytes.strip_prefix(b"-").unwrap_or(bytes);
    !digits.is_empty() && digits.iter().all(u8::is_ascii_digit)
}

fn read_line(input: &[u8]) -> Result<(&[u8], &[u8]), RespError> {
    let mut i = 0;
    while i + 1 < input.len() {
        if input[i] == b'\r' && input[i + 1] == b'\n' {
            return Ok((&input[..i], &input[i + 2..]));
        }
        i += 1;
    }
    Err(RespError::UnexpectedEof)
}

fn read_exact(input: &[u8], n: usize) -> Result<(&[u8], &[u8]), RespError> {
    if input.len() < n + 2 {
        return Err(RespError::UnexpectedEof);
    }
    let (body, rest) = input.split_at(n);
    if rest[0] != b'\r' || rest[1] != b'\n' {
        return Err(RespError::MissingTerminator);
    }
    Ok((body, &rest[2..]))
}

fn parse_n_values(mut rest: &[u8], n: usize) -> Result<(Vec<RespValue>, &[u8]), RespError> {
    let mut items = Vec::with_capacity(n);
    for _ in 0..n {
        let (value, remainder) = parse_value(rest)?;
        items.push(value);
        rest = remainder;
    }
    Ok((items, rest))
}

fn parse_n_pairs(
    mut rest: &[u8],
    n: usize,
) -> Result<(Vec<(RespValue, RespValue)>, &[u8]), RespError> {
    let mut pairs = Vec::with_capacity(n);
    for _ in 0..n {
        let (key, remainder) = parse_value(rest)?;
        let (value, remainder) = parse_value(remainder)?;
        pairs.push((key, value));
        rest = remainder;
    }
    Ok((pairs, rest))
}

fn parse_value(input: &[u8]) -> Result<(RespValue, &[u8]), RespError> {
    let (&tag, rest) = input.split_first().ok_or(RespError::UnexpectedEof)?;
    match RespType::try_from(tag)? {
        RespType::Simple(t) => parse_simple(t, rest),
        RespType::Aggregate(t) => parse_aggregate(t, rest),
    }
}

fn parse_simple(t: SimpleType, rest: &[u8]) -> Result<(RespValue, &[u8]), RespError> {
    let (line, rest) = read_line(rest)?;

    let value = match t {
        SimpleType::SimpleString => SimpleValue::SimpleString(line.to_vec()),
        SimpleType::Error => SimpleValue::Error(line.to_vec()),
        SimpleType::Integer => SimpleValue::Integer(parse_i64(line)?),
        SimpleType::Null => {
            if !line.is_empty() {
                return Err(RespError::InvalidNull(line.to_vec()));
            }
            SimpleValue::Null
        }
        SimpleType::Boolean => match line {
            b"t" => SimpleValue::Boolean(true),
            b"f" => SimpleValue::Boolean(false),
            other => return Err(RespError::InvalidBoolean(other.to_vec())),
        },
        SimpleType::Double => SimpleValue::Double(parse_f64(line)?),
        SimpleType::BigNumber => {
            if !is_valid_big_number(line) {
                return Err(RespError::InvalidBigNumber(line.to_vec()));
            }
            SimpleValue::BigNumber(line.to_vec())
        }
    };

    Ok((RespValue::Simple(value), rest))
}

fn parse_aggregate(t: AggregateType, rest: &[u8]) -> Result<(RespValue, &[u8]), RespError> {
    match t {
        AggregateType::BulkString => {
            let (len_line, rest) = read_line(rest)?;
            match parse_length(len_line, true)? {
                Length::Null => Ok((RespValue::Simple(SimpleValue::Null), rest)),
                Length::Known(n) => {
                    let (bytes, rest) = read_exact(rest, n)?;
                    Ok((
                        RespValue::Aggregate(AggregateValue::BulkString(bytes.to_vec())),
                        rest,
                    ))
                }
            }
        }
        AggregateType::Array => {
            let (len_line, rest) = read_line(rest)?;
            match parse_length(len_line, true)? {
                Length::Null => Ok((RespValue::Simple(SimpleValue::Null), rest)),
                Length::Known(n) => {
                    let (items, rest) = parse_n_values(rest, n)?;
                    Ok((RespValue::Aggregate(AggregateValue::Array(items)), rest))
                }
            }
        }
        AggregateType::BulkError => {
            let (len_line, rest) = read_line(rest)?;
            let Length::Known(n) = parse_length(len_line, false)? else {
                unreachable!("legacy null is only ever returned when explicitly allowed")
            };
            let (bytes, rest) = read_exact(rest, n)?;
            Ok((
                RespValue::Aggregate(AggregateValue::BulkError(bytes.to_vec())),
                rest,
            ))
        }
        AggregateType::VerbatimString => {
            let (len_line, rest) = read_line(rest)?;
            let Length::Known(n) = parse_length(len_line, false)? else {
                unreachable!("legacy null is only ever returned when explicitly allowed")
            };
            let (bytes, rest) = read_exact(rest, n)?;
            if n < 4 || bytes[3] != b':' {
                return Err(RespError::InvalidVerbatimString);
            }
            let mut encoding = [0u8; 3];
            encoding.copy_from_slice(&bytes[0..3]);
            let text = bytes[4..].to_vec();
            Ok((
                RespValue::Aggregate(AggregateValue::VerbatimString { encoding, text }),
                rest,
            ))
        }
        AggregateType::Map => {
            let (len_line, rest) = read_line(rest)?;
            let Length::Known(n) = parse_length(len_line, false)? else {
                unreachable!("legacy null is only ever returned when explicitly allowed")
            };
            let (pairs, rest) = parse_n_pairs(rest, n)?;
            Ok((RespValue::Aggregate(AggregateValue::Map(pairs)), rest))
        }
        AggregateType::Attribute => {
            let (len_line, rest) = read_line(rest)?;
            let Length::Known(n) = parse_length(len_line, false)? else {
                unreachable!("legacy null is only ever returned when explicitly allowed")
            };
            let (attributes, rest) = parse_n_pairs(rest, n)?;
            let (value, rest) = parse_value(rest)?;
            Ok((
                RespValue::Aggregate(AggregateValue::Attribute {
                    attributes,
                    value: Box::new(value),
                }),
                rest,
            ))
        }
        AggregateType::Set => {
            let (len_line, rest) = read_line(rest)?;
            let Length::Known(n) = parse_length(len_line, false)? else {
                unreachable!("legacy null is only ever returned when explicitly allowed")
            };
            let (items, rest) = parse_n_values(rest, n)?;
            Ok((RespValue::Aggregate(AggregateValue::Set(items)), rest))
        }
        AggregateType::Push => {
            let (len_line, rest) = read_line(rest)?;
            let Length::Known(n) = parse_length(len_line, false)? else {
                unreachable!("legacy null is only ever returned when explicitly allowed")
            };
            let (items, rest) = parse_n_values(rest, n)?;
            Ok((RespValue::Aggregate(AggregateValue::Push(items)), rest))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_deserializes(input: &[u8], expected: RespValue) {
        let value = RespValue::deserialize(input).expect("expected input to deserialize");
        assert_eq!(value, expected);
    }

    fn assert_deserialize_fails(input: &[u8], expected: RespError) {
        let err =
            RespValue::deserialize(input).expect_err("expected input to fail to deserialize");
        assert_eq!(err, expected);
    }

    fn bulk(s: &str) -> RespValue {
        RespValue::Aggregate(AggregateValue::BulkString(s.as_bytes().to_vec()))
    }

    fn bulk_bytes(bytes: &[u8]) -> RespValue {
        RespValue::Aggregate(AggregateValue::BulkString(bytes.to_vec()))
    }

    #[test]
    fn deserializes_simple_string() {
        assert_deserializes(
            b"+OK\r\n",
            RespValue::Simple(SimpleValue::SimpleString(b"OK".to_vec())),
        );
    }

    #[test]
    fn deserializes_simple_string_containing_spaces() {
        assert_deserializes(
            b"+hello world\r\n",
            RespValue::Simple(SimpleValue::SimpleString(b"hello world".to_vec())),
        );
    }

    #[test]
    fn deserializes_error() {
        assert_deserializes(
            b"-Error message\r\n",
            RespValue::Simple(SimpleValue::Error(b"Error message".to_vec())),
        );
    }

    #[test]
    fn deserializes_empty_bulk_string() {
        assert_deserializes(
            b"$0\r\n\r\n",
            RespValue::Aggregate(AggregateValue::BulkString(Vec::new())),
        );
    }

    #[test]
    fn legacy_null_bulk_string_normalizes_to_null() {
        assert_deserializes(b"$-1\r\n", RespValue::Simple(SimpleValue::Null));
    }

    #[test]
    fn legacy_null_array_normalizes_to_null() {
        assert_deserializes(b"*-1\r\n", RespValue::Simple(SimpleValue::Null));
    }

    #[test]
    fn canonical_null_deserializes() {
        assert_deserializes(b"_\r\n", RespValue::Simple(SimpleValue::Null));
    }

    #[test]
    fn deserializes_ping_command() {
        assert_deserializes(
            b"*1\r\n$4\r\nping\r\n",
            RespValue::Aggregate(AggregateValue::Array(vec![bulk("ping")])),
        );
    }

    #[test]
    fn deserializes_echo_command() {
        assert_deserializes(
            b"*2\r\n$4\r\necho\r\n$11\r\nhello world\r\n",
            RespValue::Aggregate(AggregateValue::Array(vec![
                bulk("echo"),
                bulk("hello world"),
            ])),
        );
    }

    #[test]
    fn deserializes_get_command() {
        assert_deserializes(
            b"*2\r\n$3\r\nget\r\n$3\r\nkey\r\n",
            RespValue::Aggregate(AggregateValue::Array(vec![bulk("get"), bulk("key")])),
        );
    }

    #[test]
    fn deserializes_nested_array() {
        assert_deserializes(
            b"*1\r\n*1\r\n+ok\r\n",
            RespValue::Aggregate(AggregateValue::Array(vec![RespValue::Aggregate(
                AggregateValue::Array(vec![RespValue::Simple(SimpleValue::SimpleString(
                    b"ok".to_vec(),
                ))]),
            )])),
        );
    }

    #[test]
    fn bulk_string_is_binary_safe() {
        assert_deserializes(b"$6\r\nfo\r\nob\r\n", bulk_bytes(b"fo\r\nob"));
    }

    #[test]
    fn deserializes_boolean_true() {
        assert_deserializes(b"#t\r\n", RespValue::Simple(SimpleValue::Boolean(true)));
    }

    #[test]
    fn deserializes_boolean_false() {
        assert_deserializes(b"#f\r\n", RespValue::Simple(SimpleValue::Boolean(false)));
    }

    #[test]
    fn deserializes_double() {
        assert_deserializes(b",3.14\r\n", RespValue::Simple(SimpleValue::Double(3.14)));
    }

    #[test]
    fn deserializes_double_infinity() {
        assert_deserializes(
            b",inf\r\n",
            RespValue::Simple(SimpleValue::Double(f64::INFINITY)),
        );
    }

    #[test]
    fn deserializes_big_number() {
        assert_deserializes(
            b"(3492890328409238509324850943850943825024385\r\n",
            RespValue::Simple(SimpleValue::BigNumber(
                b"3492890328409238509324850943850943825024385".to_vec(),
            )),
        );
    }

    #[test]
    fn deserializes_map() {
        assert_deserializes(
            b"%1\r\n+key\r\n:1\r\n",
            RespValue::Aggregate(AggregateValue::Map(vec![(
                RespValue::Simple(SimpleValue::SimpleString(b"key".to_vec())),
                RespValue::Simple(SimpleValue::Integer(1)),
            )])),
        );
    }

    #[test]
    fn deserializes_set() {
        assert_deserializes(
            b"~2\r\n:1\r\n:2\r\n",
            RespValue::Aggregate(AggregateValue::Set(vec![
                RespValue::Simple(SimpleValue::Integer(1)),
                RespValue::Simple(SimpleValue::Integer(2)),
            ])),
        );
    }

    #[test]
    fn deserializes_push() {
        assert_deserializes(
            b">1\r\n+message\r\n",
            RespValue::Aggregate(AggregateValue::Push(vec![RespValue::Simple(
                SimpleValue::SimpleString(b"message".to_vec()),
            )])),
        );
    }

    #[test]
    fn deserializes_verbatim_string() {
        assert_deserializes(
            b"=15\r\ntxt:Some string\r\n",
            RespValue::Aggregate(AggregateValue::VerbatimString {
                encoding: *b"txt",
                text: b"Some string".to_vec(),
            }),
        );
    }

    #[test]
    fn deserializes_attribute_wrapping_its_value() {
        assert_deserializes(
            b"|1\r\n+key\r\n+value\r\n+ok\r\n",
            RespValue::Aggregate(AggregateValue::Attribute {
                attributes: vec![(
                    RespValue::Simple(SimpleValue::SimpleString(b"key".to_vec())),
                    RespValue::Simple(SimpleValue::SimpleString(b"value".to_vec())),
                )],
                value: Box::new(RespValue::Simple(SimpleValue::SimpleString(
                    b"ok".to_vec(),
                ))),
            }),
        );
    }

    #[test]
    fn unknown_type_byte_is_an_error() {
        assert_deserialize_fails(b"^ok\r\n", RespError::UnknownType(b'^'));
    }

    #[test]
    fn non_numeric_length_is_an_error() {
        assert_deserialize_fails(b"$abc\r\n", RespError::InvalidLength(b"abc".to_vec()));
    }

    #[test]
    fn negative_length_other_than_sentinel_is_an_error() {
        assert_deserialize_fails(b"$-5\r\n", RespError::InvalidLength(b"-5".to_vec()));
    }

    #[test]
    fn bulk_string_shorter_than_declared_length_is_an_error() {
        assert_deserialize_fails(b"$5\r\nhi\r\n", RespError::UnexpectedEof);
    }

    #[test]
    fn bulk_string_missing_trailing_terminator_is_an_error() {
        assert_deserialize_fails(b"$2\r\nhiXX", RespError::MissingTerminator);
    }

    #[test]
    fn trailing_bytes_after_a_complete_value_is_an_error() {
        assert_deserialize_fails(b"+OK\r\n+OK\r\n", RespError::TrailingData);
    }

    #[test]
    fn empty_input_is_an_error() {
        assert_deserialize_fails(b"", RespError::UnexpectedEof);
    }
}
