use durabletask_client::Client;
use durabletask_sdk::types::{
    ActivityContext, ActivityOptions, ActivityResult, OrchestratorContext, OrchestratorResult,
    OrchestratorResultValue,
};
use durabletask_sdk::worker::Worker;
use futures_util::future::FutureExt;
use futures_util::join;
use std::time::Duration;
use tracing::{debug, info};
use tracing_subscriber::filter::FilterExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // initialize tracing
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "parallel=debug,durabletask_sdk=debug".into());
    let replay_filter = durabletask_sdk::filter::ReplayFilter::new();
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_filter(replay_filter))
        .init();
    let host = "http://localhost:3501";
    let mut client = Client::new(host).await?;

    tokio::spawn(async move {
        let mut worker = Worker::new(host);
        worker.add_orchestrator("parallel_orchestration", parallel_orchestration);
        worker.add_activity("test_activity", test_activity);
        worker.start().await.expect("Unable to start worker");
    });

    /*
    let id = client
        .terminate_orchestration("test_id".to_string())
        .await?;
    debug!("Instance terminated {:?}", id);

    */

    /*
    let id = client.purge_orchestration("test_id5".to_string()).await?;
    debug!("Instance purged {:?}", id);

     */

    let id = client
        .schedule_new_orchestration("test_id6".to_string(), "parallel_orchestration".to_string())
        .await?;
    debug!("Instance created {:?}", id);

    tokio::time::sleep(Duration::from_secs(60)).await;
    Ok(())
}

async fn parallel_orchestration(ctx: OrchestratorContext) -> OrchestratorResult<()> {
    info!("Parallel orchestration started");
    let activity1 = ctx.call_activity(
        ActivityOptions {
            activity_type: "test_activity".to_string(),
        },
        test_activity,
        format!("test {}", 1),
    );
    let activity2 = ctx.call_activity(
        ActivityOptions {
            activity_type: "test_activity".to_string(),
        },
        test_activity,
        format!("test {}", 2),
    );
    let (output1, output2) = join!(activity1, activity2);
    info!(
        "Parallel orchestration completed: {:?} {:?}",
        output1, output2
    );
    Ok(().into())
}

async fn test_activity(ctx: ActivityContext, input: String) -> ActivityResult<String> {
    debug!("Activity executing");
    Ok(input.to_string())
}
