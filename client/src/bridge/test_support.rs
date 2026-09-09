use std::net::SocketAddr;
use tokio::sync::oneshot;
use tokio_tungstenite::{tungstenite::protocol::Role, WebSocketStream};

pub(crate) async fn raw_server() -> (
    SocketAddr,
    oneshot::Receiver<WebSocketStream<tokio::net::TcpStream>>,
) {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        tx.send(WebSocketStream::from_raw_socket(stream, Role::Server, None).await)
            .ok();
    });
    (address, rx)
}
