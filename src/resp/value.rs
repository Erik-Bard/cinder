#[derive(Debug, Clone, PartialEq)]
pub enum RespValue {
    Simple(SimpleValue),
    Aggregate(AggregateValue),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SimpleValue {
    SimpleString(Vec<u8>),
    Error(Vec<u8>),
    Integer(i64),
    Null,
    Boolean(bool),
    Double(f64),
    BigNumber(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AggregateValue {
    BulkString(Vec<u8>),
    Array(Vec<RespValue>),
    BulkError(Vec<u8>),
    VerbatimString {
        encoding: [u8; 3],
        text: Vec<u8>,
    },
    Map(Vec<(RespValue, RespValue)>),
    Attribute {
        attributes: Vec<(RespValue, RespValue)>,
        value: Box<RespValue>,
    },
    Set(Vec<RespValue>),
    Push(Vec<RespValue>),
}

impl RespValue {
    /// Builds a Redis command: an array of bulk strings, one per word.
    /// Example: `["SET", "key", "value"]`
    pub fn command(words: &[&str]) -> RespValue {
        let parts = words
            .iter()
            .map(|word| RespValue::Aggregate(AggregateValue::BulkString(word.as_bytes().to_vec())))
            .collect();
        RespValue::Aggregate(AggregateValue::Array(parts))
    }
}
