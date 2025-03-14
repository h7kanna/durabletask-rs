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

/// Activity Environment
#[derive(Debug)]
pub struct Environment {
    pub workspace: String,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // initialize tracing
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "activity_sequence=debug,durabletask_sdk=info".into());
    let replay_filter = durabletask_sdk::filter::ReplayFilter::new();
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_filter(replay_filter))
        .init();
    let host = "http://localhost:3501";
    let mut client = Client::new(host).await?;

    tokio::spawn(async move {
        let mut worker = Worker::new(host);
        worker.add_environment(Environment {
            workspace: ".".to_string(),
        });
        worker.add_orchestrator("sequence_orchestration", sequence_orchestration);
        worker.add_activity("test_activity", test_activity);
        worker.start().await.expect("Unable to start worker");
    });

    /*
    let id = client
        .terminate_orchestration("test_id".to_string())
        .await?;
    debug!("Instance terminated {:?}", id);

    */

    let id = client
        .get_orchestration_state("test_id4".to_string(), true)
        .await?;
    if let Some(state) = id {
        debug!("Instance state {:?}", state);
    }

    let id = client.purge_orchestration("test_id4".to_string()).await?;
    debug!("Instance purged {:?}", id);

    let id = client
        .schedule_new_orchestration::<String>(
            "sequence_orchestration".to_string(),
            None,
            "test_id4".to_string(),
        )
        .await?;
    debug!("Instance created {:?}", id);

    tokio::time::sleep(Duration::from_secs(60)).await;
    Ok(())
}

async fn sequence_orchestration(ctx: OrchestratorContext) -> OrchestratorResult<()> {
    info!("Sequence orchestration started");
    let _ = ctx.create_timer(10000).await;
    let _ = ctx
        .call_activity(
            test_activity,
            ActivityOptions {
                activity_type: "test_activity".to_string(),
            },
            "test".into(),
        )
        .await;
    info!("Sequence orchestration completed");
    Ok(OrchestratorResultValue::Output(()))
}

async fn test_activity(ctx: ActivityContext, input: String) -> ActivityResult<String> {
    let environment: Option<&Environment> = ctx.environment();
    debug!(
        "Activity executing: input: {}, environment {:?}",
        input, environment
    );
    Ok("done".to_string())
}
