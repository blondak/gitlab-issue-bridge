mod app;
mod heartbeat;
mod handlers;
mod queue;
mod schema;
mod state;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "issuehub_worker=info".into()),
        )
        .init();

    app::run().await
}
