use durabletask_client::Client;
use durabletask_sdk::types::{
    ActivityContext, ActivityOptions, ActivityResult, OrchestratorContext, OrchestratorResult,
    OrchestratorResultValue,
};
use durabletask_sdk::worker::Worker;
use std::time::Duration;
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
    let host = "http://localhost:49287";
    let mut client = Client::new(host.to_string()).await?;

    tokio::spawn(async move {
        let mut worker = Worker::new(host);
        worker.add_orchestrator("test", orchestration);
        worker.add_activity("test", activity);
        worker.start().await.expect("Unable to start worker");
    });

    let id = client
        .terminate_orchestration("test_id".to_string())
        .await?;
    println!("Instance terminated {:?}", id);

    let id = client.purge_orchestration("test_id".to_string()).await?;
    println!("Instance purged {:?}", id);

    let id = client
        .schedule_new_orchestration("test_id".to_string(), "test".to_string())
        .await?;
    println!("Instance created {:?}", id);

    tokio::time::sleep(Duration::from_secs(60)).await;
    Ok(())
}

async fn orchestration(ctx: OrchestratorContext) -> OrchestratorResult<()> {
    let _ = ctx
        .call_activity(
            ActivityOptions {
                activity_type: "activity".to_string(),
            },
            activity,
            "test".into(),
        )
        .await;
    Ok(OrchestratorResultValue::Output(()))
}

async fn activity(ctx: ActivityContext, input: String) -> ActivityResult<()> {
    println!("Activity");
    Ok(())
}
