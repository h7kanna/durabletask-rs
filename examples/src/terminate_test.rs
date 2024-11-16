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
        .unwrap_or_else(|_| "terminate_test=debug,durabletask_sdk=debug".into());
    let replay_filter = durabletask_sdk::filter::ReplayFilter::new();
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_filter(replay_filter))
        .init();
    let host = "http://localhost:3501";
    let mut client = Client::new(host).await?;

    tokio::spawn(async move {
        let mut worker = Worker::new(host);
        worker.add_orchestrator("terminate_test", terminate_test);
        worker.start().await.expect("Unable to start worker");
    });

    let id = client
        .schedule_new_orchestration("test_id9".to_string(), "terminate_test".to_string())
        .await?;
    debug!("Instance created {:?}", id);

    tokio::time::sleep(Duration::from_secs(2)).await;

    let id = client
        .terminate_orchestration("test_id9".to_string(), true)
        .await?;
    debug!("Instance terminated {:?}", id);

    tokio::time::sleep(Duration::from_secs(60)).await;
    Ok(())
}

async fn terminate_test(ctx: OrchestratorContext) -> OrchestratorResult<()> {
    info!("Terminate orchestration started");
    let _ = ctx.create_timer(30000).await;
    info!("Terminate orchestration completed");
    Ok(OrchestratorResultValue::Output(()))
}
