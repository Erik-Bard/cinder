use super::error::RespError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RespVersion {
    Resp2,
    Resp3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RespCategory {
    Simple,
    Aggregate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimpleType {
    SimpleString,
    Error,
    Integer,
    Null,
    Boolean,
    Double,
    BigNumber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateType {
    BulkString,
    Array,
    BulkError,
    VerbatimString,
    Map,
    Attribute,
    Set,
    Push,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RespType {
    Simple(SimpleType),
    Aggregate(AggregateType),
}

impl RespType {
    pub fn category(&self) -> RespCategory {
        match self {
            RespType::Simple(_) => RespCategory::Simple,
            RespType::Aggregate(_) => RespCategory::Aggregate,
        }
    }

    pub fn min_version(&self) -> RespVersion {
        use RespType::*;
        match self {
            Simple(SimpleType::SimpleString | SimpleType::Error | SimpleType::Integer)
            | Aggregate(AggregateType::BulkString | AggregateType::Array) => RespVersion::Resp2,
            _ => RespVersion::Resp3,
        }
    }
}

impl TryFrom<u8> for RespType {
    type Error = RespError;

    fn try_from(value: u8) -> Result<Self, RespError> {
        use AggregateType::*;
        use SimpleType::*;

        Ok(match value {
            b'+' => RespType::Simple(SimpleString),
            b'-' => RespType::Simple(Error),
            b':' => RespType::Simple(Integer),
            b'_' => RespType::Simple(Null),
            b'#' => RespType::Simple(Boolean),
            b',' => RespType::Simple(Double),
            b'(' => RespType::Simple(BigNumber),

            b'$' => RespType::Aggregate(BulkString),
            b'*' => RespType::Aggregate(Array),
            b'!' => RespType::Aggregate(BulkError),
            b'=' => RespType::Aggregate(VerbatimString),
            b'%' => RespType::Aggregate(Map),
            b'|' => RespType::Aggregate(Attribute),
            b'~' => RespType::Aggregate(Set),
            b'>' => RespType::Aggregate(Push),

            other => return Err(RespError::UnknownType(other)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_string_tag() {
        let value = RespType::try_from(b'+').expect("expected a parsable simple string value");
        assert_eq!(value, RespType::Simple(SimpleType::SimpleString));
    }

    #[test]
    fn parse_error_tag() {
        let value = RespType::try_from(b'-').expect("expected a parsable error value");
        assert_eq!(value, RespType::Simple(SimpleType::Error));
    }

    #[test]
    fn parse_integer_tag() {
        let value = RespType::try_from(b':').expect("expected a parsable integer value");
        assert_eq!(value, RespType::Simple(SimpleType::Integer));
    }

    #[test]
    fn parse_null_tag() {
        let value = RespType::try_from(b'_').expect("expected a parsable null value");
        assert_eq!(value, RespType::Simple(SimpleType::Null));
    }

    #[test]
    fn parse_boolean_tag() {
        let value = RespType::try_from(b'#').expect("expected a parsable boolean value");
        assert_eq!(value, RespType::Simple(SimpleType::Boolean));
    }

    #[test]
    fn parse_double_tag() {
        let value = RespType::try_from(b',').expect("expected a parsable double value");
        assert_eq!(value, RespType::Simple(SimpleType::Double));
    }

    #[test]
    fn parse_big_number_tag() {
        let value = RespType::try_from(b'(').expect("expected a parsable big number value");
        assert_eq!(value, RespType::Simple(SimpleType::BigNumber));
    }

    #[test]
    fn parse_bulk_string_tag() {
        let value = RespType::try_from(b'$').expect("expected a parsable bulk string value");
        assert_eq!(value, RespType::Aggregate(AggregateType::BulkString));
    }

    #[test]
    fn parse_array_tag() {
        let value = RespType::try_from(b'*').expect("expected a parsable array value");
        assert_eq!(value, RespType::Aggregate(AggregateType::Array));
    }

    #[test]
    fn parse_bulk_error_tag() {
        let value = RespType::try_from(b'!').expect("expected a parsable bulk error value");
        assert_eq!(value, RespType::Aggregate(AggregateType::BulkError));
    }

    #[test]
    fn parse_verbatim_string_tag() {
        let value = RespType::try_from(b'=').expect("expected a parsable verbatim string value");
        assert_eq!(value, RespType::Aggregate(AggregateType::VerbatimString));
    }

    #[test]
    fn parse_map_tag() {
        let value = RespType::try_from(b'%').expect("expected a parsable map value");
        assert_eq!(value, RespType::Aggregate(AggregateType::Map));
    }

    #[test]
    fn parse_attribute_tag() {
        let value = RespType::try_from(b'|').expect("expected a parsable attribute value");
        assert_eq!(value, RespType::Aggregate(AggregateType::Attribute));
    }

    #[test]
    fn parse_set_tag() {
        let value = RespType::try_from(b'~').expect("expected a parsable set value");
        assert_eq!(value, RespType::Aggregate(AggregateType::Set));
    }

    #[test]
    fn parse_push_tag() {
        let value = RespType::try_from(b'>').expect("expected a parsable push value");
        assert_eq!(value, RespType::Aggregate(AggregateType::Push));
    }

    #[test]
    fn parse_unknown_byte_is_an_error() {
        let err = RespType::try_from(b'x').expect_err("expected an unknown type byte to error");
        assert_eq!(err, RespError::UnknownType(b'x'));
    }

    #[test]
    fn simple_types_report_simple_category() {
        assert_eq!(
            RespType::Simple(SimpleType::Integer).category(),
            RespCategory::Simple
        );
    }

    #[test]
    fn aggregate_types_report_aggregate_category() {
        assert_eq!(
            RespType::Aggregate(AggregateType::Array).category(),
            RespCategory::Aggregate
        );
    }

    #[test]
    fn resp2_types_report_resp2_minimum_version() {
        assert_eq!(
            RespType::Simple(SimpleType::SimpleString).min_version(),
            RespVersion::Resp2
        );
        assert_eq!(
            RespType::Simple(SimpleType::Error).min_version(),
            RespVersion::Resp2
        );
        assert_eq!(
            RespType::Simple(SimpleType::Integer).min_version(),
            RespVersion::Resp2
        );
        assert_eq!(
            RespType::Aggregate(AggregateType::BulkString).min_version(),
            RespVersion::Resp2
        );
        assert_eq!(
            RespType::Aggregate(AggregateType::Array).min_version(),
            RespVersion::Resp2
        );
    }

    #[test]
    fn resp3_types_report_resp3_minimum_version() {
        assert_eq!(
            RespType::Simple(SimpleType::Null).min_version(),
            RespVersion::Resp3
        );
        assert_eq!(
            RespType::Simple(SimpleType::Boolean).min_version(),
            RespVersion::Resp3
        );
        assert_eq!(
            RespType::Simple(SimpleType::Double).min_version(),
            RespVersion::Resp3
        );
        assert_eq!(
            RespType::Simple(SimpleType::BigNumber).min_version(),
            RespVersion::Resp3
        );
        assert_eq!(
            RespType::Aggregate(AggregateType::BulkError).min_version(),
            RespVersion::Resp3
        );
        assert_eq!(
            RespType::Aggregate(AggregateType::VerbatimString).min_version(),
            RespVersion::Resp3
        );
        assert_eq!(
            RespType::Aggregate(AggregateType::Map).min_version(),
            RespVersion::Resp3
        );
        assert_eq!(
            RespType::Aggregate(AggregateType::Attribute).min_version(),
            RespVersion::Resp3
        );
        assert_eq!(
            RespType::Aggregate(AggregateType::Set).min_version(),
            RespVersion::Resp3
        );
        assert_eq!(
            RespType::Aggregate(AggregateType::Push).min_version(),
            RespVersion::Resp3
        );
    }
}
