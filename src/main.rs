use std::io;

const ADDR: &str = "127.0.0.1:6379";

#[tokio::main]
async fn main() -> io::Result<()> {
    cinder::server::run(ADDR).await
}
