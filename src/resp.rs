#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RespSum {
    SimpleString,
    Error,
    Integer,
    BulkString,
    Array,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RespError {
    UnknownType(u8),
}

impl TryFrom<u8> for RespSum {
    type Error = RespError;

    fn try_from(value: u8) -> Result<Self, RespError> {
        match value {
            b'+' => Ok(RespSum::SimpleString),
            b'-' => Ok(RespSum::Error),
            b':' => Ok(RespSum::Integer),
            b'$' => Ok(RespSum::BulkString),
            b'*' => Ok(RespSum::Array),
            other => Err(RespError::UnknownType(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_string() {
        let input: u8 = b'+';
        let value = RespSum::try_from(input).expect("Expect a parsable simple string value");
        assert_eq!(value, RespSum::SimpleString);
    }

    #[test]
    fn parse_error() {
        let input: u8 = b'-';
        let value = RespSum::try_from(input).expect("Expect a parsable error value");
        assert_eq!(value, RespSum::Error);
    }

    #[test]
    fn parse_integer() {
        let input: u8 = b':';
        let value = RespSum::try_from(input).expect("Expect a parsable integer value");
        assert_eq!(value, RespSum::Integer);
    }

    #[test]
    fn parse_bulk_string() {
        let input: u8 = b'$';
        let value = RespSum::try_from(input).expect("Expect a parsable bulk string value");
        assert_eq!(value, RespSum::BulkString);
    }

    #[test]
    fn parse_array() {
        let input: u8 = b'*';
        let value = RespSum::try_from(input).expect("Expect a parsable array value");
        assert_eq!(value, RespSum::Array);
    }

    #[test]
    fn parse_unknown_byte_is_an_error() {
        let input: u8 = b'x';
        let err = RespSum::try_from(input).expect_err("expected an unknown type byte to error");
        assert_eq!(err, RespError::UnknownType(b'x'));
    }
}
