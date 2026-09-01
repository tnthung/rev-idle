mod console;
mod script;
mod udp;

use std::io;
use tokio::sync::watch;

#[tokio::main]
async fn main() -> io::Result<()> {
    let (state_tx, mut state_rx) = watch::channel(udp::State::default());
    let receiver = tokio::spawn(udp::run(state_tx));

    while state_rx.changed().await.is_ok() {
        println!("{:?}", state_rx.borrow_and_update());
    }

    receiver
        .await
        .map_err(|error| io::Error::other(error.to_string()))?
}
