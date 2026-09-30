use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};

use crate::resp::{READ_CHUNK_SIZE, RespValue, SimpleValue};

/// Entry point for the `cinder-cli` binary (src/bin/cinder-cli.rs).
pub fn run() -> io::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 6379));
    let mut stream = TcpStream::connect(addr)?;

    let request = RespValue::command(&["PING"]);
    stream.write_all(&request.serialize())?;

    let reply = read_reply(&mut stream)?;
    let str_reply = format_reply(reply);
    println!("{}", str_reply);
    Ok(())
}

fn read_reply(reader: &mut impl Read) -> io::Result<RespValue> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; READ_CHUNK_SIZE];

    loop {
        let parsed = RespValue::next_frame(&buffer)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, format!("{err:?}")))?;
        if let Some((value, _consumed)) = parsed {
            return Ok(value);
        }

        let n = reader.read(&mut chunk)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "server closed the connection before sending a full reply",
            ));
        }
        buffer.extend_from_slice(&chunk[..n]);
    }
}

fn format_reply(val: RespValue) -> String {
    match &val {
        RespValue::Simple(SimpleValue::SimpleString(bytes)) => {
            String::from_utf8_lossy(bytes).to_string()
        }
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resp::SimpleValue;
    use std::io::Cursor;

    struct Trickle {
        data: Vec<u8>,
        pos: usize,
        step: usize,
    }

    impl Read for Trickle {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let end = (self.pos + self.step).min(self.data.len()).min(self.pos + buf.len());
            let n = end - self.pos;
            buf[..n].copy_from_slice(&self.data[self.pos..end]);
            self.pos = end;
            Ok(n)
        }
    }

    fn pong() -> RespValue {
        RespValue::Simple(SimpleValue::SimpleString(b"PONG".to_vec()))
    }

    #[test]
    fn reads_a_complete_reply() {
        let mut reader = Cursor::new(b"+PONG\r\n".to_vec());
        assert_eq!(read_reply(&mut reader).unwrap(), pong());
    }

    #[test]
    fn reads_a_reply_that_arrives_in_pieces() {
        let mut reader = Trickle { data: b"+PONG\r\n".to_vec(), pos: 0, step: 2 };
        assert_eq!(read_reply(&mut reader).unwrap(), pong());
    }

    #[test]
    fn connection_closed_mid_reply_is_an_error() {
        let mut reader = Cursor::new(b"+PO".to_vec());
        let err = read_reply(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn invalid_reply_is_an_error() {
        let mut reader = Cursor::new(b"^nope\r\n".to_vec());
        let err = read_reply(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
