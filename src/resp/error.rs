#[derive(Debug, PartialEq, Eq)]
pub enum RespError {
    UnknownType(u8),
    UnexpectedEof,
    MissingTerminator,
    TrailingData,
    InvalidInteger(Vec<u8>),
    InvalidLength(Vec<u8>),
    InvalidBoolean(Vec<u8>),
    InvalidDouble(Vec<u8>),
    InvalidBigNumber(Vec<u8>),
    InvalidNull(Vec<u8>),
    InvalidVerbatimString,
}
