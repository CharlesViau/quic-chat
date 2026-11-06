mod app;
mod chat_client;
mod console;
mod session;
mod settings;
mod tls;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
