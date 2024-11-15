use crate::executor::{ActivityExecutor, OrchestrationExecutor};
use crate::registry::Registry;
use crate::types::{ActivityFunction, IntoActivityFunc, OrchestratorFunction};
use durabletask_proto::{
    task_hub_sidecar_service_client::TaskHubSidecarServiceClient, work_item::Request,
    ActivityRequest, ActivityResponse, GetWorkItemsRequest, OrchestratorRequest,
    OrchestratorResponse,
};
use futures_util::StreamExt;
use std::sync::Arc;
use tonic::IntoRequest;
use tracing::{debug, field, info_span, Instrument};

#[derive(Default)]
pub struct Worker {
    host: String,
    registry: Registry,
}

impl Worker {
    pub fn new(host: &str) -> Self {
        Self {
            registry: Registry::default(),
            host: host.to_string(),
        }
    }

    pub async fn start(self) -> Result<(), anyhow::Error> {
        let mut client = TaskHubSidecarServiceClient::connect(self.host).await?;
        let mut stream = client
            .get_work_items(GetWorkItemsRequest {})
            .await?
            .into_inner();
        let registry = Arc::new(self.registry);
        while let Some(item) = stream.next().await {
            let mut client = client.clone();
            let registry = registry.clone();
            tokio::spawn(async move {
                match item {
                    Ok(item) => {
                        if let Some(request) = item.request {
                            match request {
                                Request::OrchestratorRequest(request) => {
                                    match Self::execute_orchestrator(registry, request).await {
                                        Ok(response) => {
                                            match client
                                                .complete_orchestrator_task(response.into_request())
                                                .await
                                            {
                                                Ok(_) => {}
                                                Err(err) => {
                                                    debug!("{:?}", err);
                                                }
                                            }
                                        }
                                        Err(err) => {
                                            debug!("Orchestrator execution failed {:?}", err);
                                        }
                                    }
                                }
                                Request::ActivityRequest(request) => {
                                    match Self::execute_activity(registry, request).await {
                                        Ok(response) => {
                                            match client
                                                .complete_activity_task(response.into_request())
                                                .await
                                            {
                                                Ok(_) => {}
                                                Err(err) => {
                                                    debug!("{:?}", err);
                                                }
                                            }
                                        }
                                        Err(err) => {
                                            debug!("Activity execution failed {:?}", err);
                                        }
                                    }
                                }
                                Request::EntityRequest(_) => {}
                                Request::HealthPing(_) => {}
                            }
                        }
                    }
                    Err(err) => {}
                }
            });
        }
        Ok(())
    }

    pub fn add_orchestrator(
        &mut self,
        name: impl Into<String>,
        func: impl Into<OrchestratorFunction>,
    ) {
        self.registry.add_orchestrator(name.into(), func.into());
    }

    pub fn add_activity<A, R, O>(
        &mut self,
        name: impl Into<String>,
        func: impl IntoActivityFunc<A, R, O>,
    ) {
        self.registry.add_activity(
            name.into(),
            ActivityFunction {
                activity_fn: func.into_activity_fn(),
            },
        );
    }

    async fn execute_orchestrator(
        registry: Arc<Registry>,
        request: OrchestratorRequest,
    ) -> Result<OrchestratorResponse, anyhow::Error> {
        debug!("Orchestrator request: {:?}", request);
        let executor = OrchestrationExecutor::new(registry);
        let mut actions;
        #[cfg(feature = "tracing-subscriber")]
        {
            actions = executor
                .execute(
                    request.instance_id.clone(),
                    request.past_events,
                    request.new_events,
                )
                .instrument(info_span!(
                    "execute",
                    "instance_id" = request.instance_id,
                    "replaying" = field::Empty,
                ))
                .await?;
        }
        #[cfg(not(feature = "tracing-subscriber"))]
        {
            actions = executor
                .execute(
                    request.instance_id.clone(),
                    request.past_events,
                    request.new_events,
                )
                .await?;
        }
        debug!("Actions issued: {:?}", actions);
        Ok(OrchestratorResponse {
            instance_id: request.instance_id,
            actions,
            custom_status: None,
        })
    }

    async fn execute_activity(
        registry: Arc<Registry>,
        request: ActivityRequest,
    ) -> Result<ActivityResponse, anyhow::Error> {
        debug!("Activity request: {:?}", request);
        let executor = ActivityExecutor::new(registry);
        if let Some(instance) = request.orchestration_instance {
            let output = executor
                .execute(
                    instance.instance_id.clone(),
                    request.name,
                    request.task_id,
                    request.input,
                )
                .await?;
            debug!("Activity executed: {:?}", output);
            return Ok(ActivityResponse {
                instance_id: instance.instance_id,
                task_id: request.task_id,
                result: output,
                failure_details: None,
            });
        }
        Ok(ActivityResponse::default())
    }
}
