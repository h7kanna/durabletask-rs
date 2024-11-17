use durabletask_client::Client;
use durabletask_sdk::types::{
    ActivityContext, ActivityOptions, ActivityResult, OrchestratorContext, OrchestratorResult,
    OrchestratorResultValue, SubOrchestratorOptions,
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
        worker.add_orchestrator("sequence_orchestration", sequence_orchestration);
        //worker.add_orchestrator("sub_sequence_orchestration", sub_sequence_orchestration);
        worker.add_activity("test_activity", test_activity);
        worker.start().await.expect("Unable to start worker");
    });

    let id = client
        .get_orchestration_state("test_id4".to_string(), true)
        .await?;
    if let Some(state) = id {
        debug!("Instance state {:?}", state);
    }

    let id = client.purge_orchestration("test_id4".to_string()).await?;
    debug!("Instance purged {:?}", id);

    let id = client
        .schedule_new_orchestration("test_id4".to_string(), "sequence_orchestration".to_string())
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
            ActivityOptions {
                activity_type: "test_activity".to_string(),
            },
            test_activity,
            "test".into(),
        )
        .await;
    let _ = ctx
        .call_sub_orchestrator(
            SubOrchestratorOptions {
                orchestrator_type: "sub_sequence_orchestration".to_string(),
                instance_id: format!("sub-{}", ctx.instance_id()),
            },
            sub_sequence_orchestration,
            "test".into(),
        )
        .await;
    info!("Sequence orchestration completed");
    Ok(OrchestratorResultValue::Output(()))
}

async fn sub_sequence_orchestration(
    ctx: OrchestratorContext,
    input: String,
) -> OrchestratorResult<()> {
    info!("Sub sequence orchestration started");
    let _ = ctx
        .call_activity(
            ActivityOptions {
                activity_type: "test_activity".to_string(),
            },
            test_activity,
            input,
        )
        .await;
    info!("Sub sequence orchestration completed");
    Ok(OrchestratorResultValue::Output(()))
}

async fn test_activity(ctx: ActivityContext, input: String) -> ActivityResult<String> {
    debug!("Activity executing");
    Ok("done".to_string())
}
