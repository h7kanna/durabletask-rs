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
                                                    println!("{:?}", err);
                                                }
                                            }
                                        }
                                        Err(err) => {}
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
                                                    println!("{:?}", err);
                                                }
                                            }
                                        }
                                        Err(err) => {}
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
        let executor = OrchestrationExecutor::new(registry);
        let actions = executor
            .execute(
                request.instance_id.clone(),
                request.past_events,
                request.new_events,
            )
            .await?;
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
