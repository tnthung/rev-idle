mod app;
mod bridge;
mod window;
mod console;
mod script;
mod hotkey;
mod capture;
mod global_state;

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    app::run().await
}
