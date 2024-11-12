use durabletask_sdk::worker::Worker;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // initialize tracing
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "activity_sequence=debug,durabletask_sdk=debug".into());
    let replay_filter = durabletask_sdk::logger::ReplayFilter::new();
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_filter(replay_filter))
        .init();
    Worker::start().await?;
    Ok(())
}

async fn orchestration() -> Result<(), anyhow::Error> {
    Ok(())
}

async fn activity() -> Result<(), anyhow::Error> {
    Ok(())
}
