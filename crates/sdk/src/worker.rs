use crate::registry::Registry;
use crate::types::{ActivityFunction, OrchestratorFunction};
use anyhow::Error;
use durabletask_proto::{
    task_hub_sidecar_service_client::TaskHubSidecarServiceClient, work_item::Request,
    ActivityRequest, ActivityResponse, GetWorkItemsRequest, OrchestratorRequest,
    OrchestratorResponse,
};
use futures_util::StreamExt;

pub struct Worker {
    registry: Registry,
}

impl Worker {
    pub async fn start() -> Result<(), anyhow::Error> {
        let mut client = TaskHubSidecarServiceClient::connect("http://localhost:54145").await?;

        let mut stream = client
            .get_work_items(GetWorkItemsRequest {})
            .await?
            .into_inner();

        while let Some(item) = stream.next().await {
            let client = client.clone();
            tokio::spawn(async move {
                match item {
                    Ok(item) => {
                        if let Some(request) = item.request {
                            match request {
                                Request::OrchestratorRequest(request) => {
                                    match Self::execute_orchestrator(client, request).await {
                                        Ok(response) => {}
                                        Err(err) => {}
                                    }
                                }
                                Request::ActivityRequest(request) => {
                                    match Self::execute_activity(client, request).await {
                                        Ok(response) => {}
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

    pub fn add_activity(&mut self, name: impl Into<String>, func: impl Into<ActivityFunction>) {
        self.registry.add_activity(name.into(), func.into());
    }

    async fn execute_orchestrator(
        client: TaskHubSidecarServiceClient<tonic::transport::Channel>,
        request: OrchestratorRequest,
    ) -> Result<OrchestratorResponse, anyhow::Error> {
        Ok(OrchestratorResponse::default())
    }

    async fn execute_activity(
        client: TaskHubSidecarServiceClient<tonic::transport::Channel>,
        request: ActivityRequest,
    ) -> Result<ActivityResponse, anyhow::Error> {
        Ok(ActivityResponse::default())
    }
}
