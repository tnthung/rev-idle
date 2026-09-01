use std::io;
use tokio::net::UdpSocket;

const LISTEN_ADDRESS: &str = "127.0.0.1:19841";

async fn receive_datagram(socket: &UdpSocket, buffer: &mut [u8]) -> io::Result<String> {
    let (length, _) = socket.recv_from(buffer).await?;
    Ok(String::from_utf8_lossy(&buffer[..length]).into_owned())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let socket = UdpSocket::bind(LISTEN_ADDRESS).await?;
    let mut buffer = [0_u8; 65_535];

    loop {
        println!("{}", receive_datagram(&socket, &mut buffer).await?);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UdpSocket;

    #[tokio::test]
    async fn receives_exact_utf8_datagram() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let payload = br#"{"score":"2.5e42"}"#;

        sender
            .send_to(payload, receiver.local_addr().unwrap())
            .await
            .unwrap();

        let mut buffer = [0_u8; 65_535];
        let received = receive_datagram(&receiver, &mut buffer).await.unwrap();
        assert_eq!(received, r#"{"score":"2.5e42"}"#);
    }

    #[tokio::test]
    async fn replaces_invalid_utf8_without_dropping_datagram() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();

        sender
            .send_to(&[b'f', 0xff, b'o'], receiver.local_addr().unwrap())
            .await
            .unwrap();

        let mut buffer = [0_u8; 65_535];
        let received = receive_datagram(&receiver, &mut buffer).await.unwrap();
        assert_eq!(received, "f\u{fffd}o");
    }
}
