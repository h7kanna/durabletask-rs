use durabletask_proto::task_hub_sidecar_service_client::TaskHubSidecarServiceClient;
use durabletask_proto::{
    purge_instances_request, CreateInstanceRequest, CreateInstanceResponse, PurgeInstancesRequest,
    PurgeInstancesResponse, RaiseEventRequest, RaiseEventResponse, TerminateRequest,
    TerminateResponse,
};

pub struct Client {
    inner: TaskHubSidecarServiceClient<tonic::transport::Channel>,
}

impl Client {
    pub async fn new(host: String) -> Result<Self, anyhow::Error> {
        let inner = TaskHubSidecarServiceClient::connect(host).await?;
        Ok(Self { inner })
    }

    pub async fn schedule_new_orchestration(
        &mut self,
        instance_id: String,
        name: String,
    ) -> Result<CreateInstanceResponse, anyhow::Error> {
        let request = CreateInstanceRequest {
            instance_id,
            name,
            version: None,
            input: None,
            scheduled_start_timestamp: None,
            orchestration_id_reuse_policy: None,
            execution_id: None,
            tags: Default::default(),
        };
        let response = self.inner.start_instance(request).await?;
        Ok(response.into_inner())
    }

    pub async fn terminate_orchestration(
        &mut self,
        instance_id: String,
    ) -> Result<TerminateResponse, anyhow::Error> {
        let request = TerminateRequest {
            instance_id,
            output: None,
            recursive: false,
        };
        let response = self.inner.terminate_instance(request).await?;
        Ok(response.into_inner())
    }

    pub async fn raise_orchestration_event(
        &mut self,
        instance_id: String,
        name: String,
        input: Option<String>,
    ) -> Result<RaiseEventResponse, anyhow::Error> {
        let request = RaiseEventRequest {
            instance_id,
            name,
            input,
        };
        let response = self.inner.raise_event(request).await?;
        Ok(response.into_inner())
    }

    pub async fn purge_orchestration(
        &mut self,
        instance_id: String,
    ) -> Result<PurgeInstancesResponse, anyhow::Error> {
        let request = PurgeInstancesRequest {
            recursive: false,
            request: Some(purge_instances_request::Request::InstanceId(instance_id)),
        };
        let response = self.inner.purge_instances(request).await?;
        Ok(response.into_inner())
    }
}
