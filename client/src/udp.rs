use serde::Deserialize;
use std::{
    io,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{net::UdpSocket, sync::watch};

pub const LISTEN_ADDRESS: &str = "127.0.0.1:19841";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub score: Option<String>,
    pub sequence: u64,
    pub received_at_ms: Option<u64>,
}

#[derive(Deserialize)]
struct ScorePayload {
    score: String,
}

async fn receive_one(
    socket: &UdpSocket,
    state_tx: &watch::Sender<State>,
    sequence: &mut u64,
    buffer: &mut [u8],
) -> io::Result<()> {
    let (length, source) = socket.recv_from(buffer).await?;
    let payload = match serde_json::from_slice::<ScorePayload>(&buffer[..length]) {
        Ok(payload) => payload,
        Err(error) => {
            eprintln!("ignored invalid telemetry from {source}: {error}");
            return Ok(());
        }
    };

    *sequence = (*sequence).saturating_add(1);
    let received_at_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX);

    state_tx
        .send(State {
            score: Some(payload.score),
            sequence: *sequence,
            received_at_ms: Some(received_at_ms),
        })
        .map_err(|_| io::Error::other("script state receiver closed"))?;

    Ok(())
}

pub async fn run(state_tx: watch::Sender<State>) -> io::Result<()> {
    let socket = UdpSocket::bind(LISTEN_ADDRESS).await?;
    let mut sequence = 0;
    let mut buffer = [0_u8; 65_535];

    loop {
        receive_one(
            &socket,
            &state_tx,
            &mut sequence,
            &mut buffer,
        )
        .await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{net::UdpSocket, sync::watch};

    async fn deliver(payload: &[u8], initial: State) -> State {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let (state_tx, state_rx) = watch::channel(initial.clone());
        let mut sequence = initial.sequence;
        let mut buffer = [0_u8; 65_535];

        sender
            .send_to(payload, receiver.local_addr().unwrap())
            .await
            .unwrap();

        receive_one(
            &receiver,
            &state_tx,
            &mut sequence,
            &mut buffer,
        )
        .await
        .unwrap();

        state_rx.borrow().clone()
    }

    #[test]
    fn default_state_is_empty() {
        assert_eq!(State::default().score, None);
        assert_eq!(State::default().sequence, 0);
        assert_eq!(State::default().received_at_ms, None);
    }

    #[tokio::test]
    async fn valid_packet_replaces_state_without_losing_score_precision() {
        let state = deliver(
            br#"{"score":"1.2345678901234567e123","extra":true}"#,
            State::default(),
        )
        .await;

        assert_eq!(
            state.score.as_deref(),
            Some("1.2345678901234567e123")
        );
        assert_eq!(state.sequence, 1);
        assert!(state.received_at_ms.is_some());
    }

    #[tokio::test]
    async fn malformed_packets_preserve_previous_state() {
        let previous = State {
            score: Some("9.5e42".to_owned()),
            sequence: 7,
            received_at_ms: Some(123),
        };

        for payload in [
            b"not-json".as_slice(),
            br#"{"score":42}"#.as_slice(),
            &[b'f', 0xff, b'o'],
        ] {
            assert_eq!(deliver(payload, previous.clone()).await, previous);
        }
    }

    #[tokio::test]
    async fn latest_valid_packet_wins_and_sequence_increments() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let (state_tx, state_rx) = watch::channel(State::default());
        let mut sequence = 0;
        let mut buffer = [0_u8; 65_535];

        for payload in [
            br#"{"score":"1e1"}"#.as_slice(),
            br#"{"score":"2e2"}"#.as_slice(),
        ] {
            sender
                .send_to(payload, receiver.local_addr().unwrap())
                .await
                .unwrap();
            receive_one(
                &receiver,
                &state_tx,
                &mut sequence,
                &mut buffer,
            )
            .await
            .unwrap();
        }

        let state = state_rx.borrow().clone();
        assert_eq!(state.score.as_deref(), Some("2e2"));
        assert_eq!(state.sequence, 2);
    }
}
