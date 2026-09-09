use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use crate::commands;
use crate::resp::{RespError, RespValue, SimpleValue};

const READ_CHUNK_SIZE: usize = 4096;

pub async fn run(addr: &str) -> io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    println!("cinder listening on {addr}");

    loop {
        let (socket, peer) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(err) = handle_connection(socket).await {
                eprintln!("connection {peer} ended with an error: {err}");
            }
        });
    }
}

async fn handle_connection(mut stream: TcpStream) -> io::Result<()> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; READ_CHUNK_SIZE];

    loop {
        loop {
            match next_frame(&buffer) {
                Ok(Some((request, consumed))) => {
                    let response = commands::dispatch(&request);
                    stream.write_all(&response.serialize()).await?;
                    buffer.drain(..consumed);
                }
                Ok(None) => break,
                Err(err) => {
                    let response = RespValue::Simple(SimpleValue::Error(
                        format!("ERR Protocol error: {err:?}").into_bytes(),
                    ));
                    stream.write_all(&response.serialize()).await?;
                    return Ok(());
                }
            }
        }

        let bytes_read = stream.read(&mut chunk).await?;
        if bytes_read == 0 {
            return Ok(()); // client closed the connection
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
    }
}

fn next_frame(buffer: &[u8]) -> Result<Option<(RespValue, usize)>, RespError> {
    if buffer.is_empty() {
        return Ok(None);
    }

    match RespValue::deserialize_prefix(buffer) {
        Ok((value, consumed)) => Ok(Some((value, consumed))),
        Err(RespError::UnexpectedEof) => Ok(None),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_frame_waits_for_more_bytes_on_an_empty_buffer() {
        assert_eq!(next_frame(b""), Ok(None));
    }

    #[test]
    fn next_frame_waits_for_more_bytes_on_a_partial_command() {
        assert_eq!(next_frame(b"*1\r\n$4\r\nPI"), Ok(None));
    }

    #[test]
    fn next_frame_returns_a_complete_command_and_its_length() {
        let (value, consumed) = next_frame(b"*1\r\n$4\r\nping\r\n")
            .expect("expected parsing to succeed")
            .expect("expected a complete command");
        assert_eq!(consumed, 14);
        assert_eq!(
            commands::dispatch(&value),
            RespValue::Simple(SimpleValue::SimpleString(b"PONG".to_vec()))
        );
    }

    #[test]
    fn next_frame_reports_genuine_protocol_errors() {
        let err = next_frame(b"^nope\r\n").expect_err("expected a protocol error");
        assert_eq!(err, RespError::UnknownType(b'^'));
    }

    #[tokio::test]
    async fn responds_to_pipelined_ping_and_echo_over_a_real_socket() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("expected to bind an ephemeral port");
        let addr = listener.local_addr().expect("expected a local address");

        tokio::spawn(async move {
            let (socket, _peer) = listener.accept().await.expect("expected a connection");
            let _ = handle_connection(socket).await;
        });

        let mut client = TcpStream::connect(addr)
            .await
            .expect("expected to connect to the server");

        client
            .write_all(b"*1\r\n$4\r\nPING\r\n*2\r\n$4\r\nECHO\r\n$5\r\nhello\r\n")
            .await
            .expect("expected the write to succeed");

        let expected = b"+PONG\r\n$5\r\nhello\r\n";
        let mut received = Vec::new();
        let mut buf = [0u8; 128];
        while received.len() < expected.len() {
            let n = client
                .read(&mut buf)
                .await
                .expect("expected to read a reply");
            assert!(n > 0, "connection closed before both replies arrived");
            received.extend_from_slice(&buf[..n]);
        }

        assert_eq!(received, expected);
    }
}
