use durabletask_proto::task_hub_sidecar_service_client::TaskHubSidecarServiceClient;
use durabletask_proto::{CreateInstanceRequest, TerminateRequest};

pub struct Client {
    inner: TaskHubSidecarServiceClient<tonic::transport::Channel>,
}

impl Client {
    pub async fn schedule_new_orchestration(
        &mut self,
        instance_id: String,
        name: String,
    ) -> Result<(), anyhow::Error> {
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
        Ok(())
    }

    pub async fn terminate_orchestration(
        &mut self,
        instance_id: String,
    ) -> Result<(), anyhow::Error> {
        let request = TerminateRequest {
            instance_id,
            output: None,
            recursive: false,
        };
        let response = self.inner.terminate_instance(request).await?;
        Ok(())
    }
}
