use durabletask_proto::task_hub_sidecar_service_client::TaskHubSidecarServiceClient;
use durabletask_proto::{
    purge_instances_request, CreateInstanceRequest, CreateInstanceResponse, GetInstanceRequest,
    OrchestrationState, PurgeInstancesRequest, PurgeInstancesResponse, RaiseEventRequest,
    RaiseEventResponse, ResumeRequest, ResumeResponse, SuspendRequest, SuspendResponse,
    TerminateRequest, TerminateResponse,
};
use serde::Serialize;

#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub enum Error {
    Serde(#[from] serde_json::Error),
    Transport(#[from] tonic::transport::Error),
    Grpc(#[from] tonic::Status),
}

#[derive(Clone)]
pub struct Client {
    inner: TaskHubSidecarServiceClient<tonic::transport::Channel>,
}

impl Client {
    pub async fn new(host: &str) -> Result<Self, Error> {
        let inner = TaskHubSidecarServiceClient::connect(host.to_string()).await?;
        Ok(Self { inner })
    }

    pub async fn schedule_new_orchestration<T: Serialize>(
        &mut self,
        name: String,
        input: Option<T>,
        instance_id: String,
    ) -> Result<CreateInstanceResponse, Error> {
        let input = if let Some(input) = input {
            Some(serde_json::to_string(&input)?)
        } else {
            None
        };
        let request = CreateInstanceRequest {
            instance_id,
            name,
            version: None,
            input,
            scheduled_start_timestamp: None,
            orchestration_id_reuse_policy: None,
            execution_id: None,
            tags: Default::default(),
        };
        let response = self.inner.start_instance(request).await?;
        Ok(response.into_inner())
    }

    pub async fn get_orchestration_state(
        &mut self,
        instance_id: String,
        fetch_payloads: bool,
    ) -> Result<Option<OrchestrationState>, Error> {
        let request = GetInstanceRequest {
            instance_id,
            get_inputs_and_outputs: fetch_payloads,
        };
        let response = self.inner.get_instance(request).await?;
        Ok(response.into_inner().orchestration_state)
    }

    pub async fn terminate_orchestration(
        &mut self,
        instance_id: String,
        recursive: bool,
    ) -> Result<TerminateResponse, Error> {
        let request = TerminateRequest {
            instance_id,
            output: None,
            recursive,
        };
        let response = self.inner.terminate_instance(request).await?;
        Ok(response.into_inner())
    }

    pub async fn raise_orchestration_event<T: Serialize>(
        &mut self,
        name: String,
        input: Option<T>,
        instance_id: String,
    ) -> Result<RaiseEventResponse, Error> {
        let input = if let Some(input) = input {
            Some(serde_json::to_string(&input)?)
        } else {
            None
        };
        let request = RaiseEventRequest {
            instance_id,
            name,
            input,
        };
        let response = self.inner.raise_event(request).await?;
        Ok(response.into_inner())
    }

    pub async fn suspend_orchestration(
        &mut self,
        instance_id: String,
    ) -> Result<SuspendResponse, Error> {
        let request = SuspendRequest {
            instance_id,
            reason: None,
        };
        let response = self.inner.suspend_instance(request).await?;
        Ok(response.into_inner())
    }

    pub async fn resume_orchestration(
        &mut self,
        instance_id: String,
    ) -> Result<ResumeResponse, Error> {
        let request = ResumeRequest {
            instance_id,
            reason: None,
        };
        let response = self.inner.resume_instance(request).await?;
        Ok(response.into_inner())
    }

    pub async fn purge_orchestration(
        &mut self,
        instance_id: String,
    ) -> Result<PurgeInstancesResponse, Error> {
        let request = PurgeInstancesRequest {
            recursive: false,
            request: Some(purge_instances_request::Request::InstanceId(instance_id)),
        };
        let response = self.inner.purge_instances(request).await?;
        Ok(response.into_inner())
    }
}
