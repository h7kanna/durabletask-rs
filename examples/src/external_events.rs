use durabletask_client::Client;
use durabletask_sdk::types::{
    ActivityContext, ActivityOptions, ActivityResult, OrchestratorContext, OrchestratorResult,
    OrchestratorResultValue,
};
use durabletask_sdk::worker::Worker;
use std::time::Duration;
use tracing::{debug, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // initialize tracing
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "external_events=debug,durabletask_sdk=debug".into());
    let replay_filter = durabletask_sdk::filter::ReplayFilter::new();
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_filter(replay_filter))
        .init();
    let host = "http://localhost:3501";
    let mut client = Client::new(host).await?;

    tokio::spawn(async move {
        let mut worker = Worker::new(host);
        worker.add_orchestrator("external_events", external_events);
        worker.start().await.expect("Unable to start worker");
    });

    let id = client.purge_orchestration("test_id45".to_string()).await?;
    debug!("Instance purged {:?}", id);

    let id = client
        .schedule_new_orchestration("test_id45".to_string(), "external_events".to_string())
        .await?;
    debug!("Instance created {:?}", id);

    tokio::time::sleep(Duration::from_secs(3)).await;

    let id = client
        .raise_orchestration_event(
            "test_id45".to_string(),
            "test_signal".to_string(),
            Some("signal_input".to_string()),
        )
        .await?;
    debug!("Raised event {:?}", id);

    tokio::time::sleep(Duration::from_secs(10)).await;
    Ok(())
}

async fn external_events(ctx: OrchestratorContext) -> OrchestratorResult<String> {
    info!("External events started");
    let result = ctx.await_signal_event("test_signal").await?;
    debug!("External events completed: {}", result);
    Ok(OrchestratorResultValue::Output(result))
}
