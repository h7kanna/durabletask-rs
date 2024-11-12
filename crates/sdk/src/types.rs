use futures_util::future::BoxFuture;
use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::mpsc::Sender;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("serialization error")]
    Serialization,
    #[error("timeout error")]
    Timeout,
    #[error("non-determinism error")]
    NonDeterminism,
}

pub trait AsJsonPayloadExt {
    fn as_json_payload(&self) -> anyhow::Result<Vec<u8>>;
}
impl<T> AsJsonPayloadExt for T
where
    T: Serialize,
{
    fn as_json_payload(&self) -> anyhow::Result<Vec<u8>> {
        let as_json = serde_json::to_string(self)?;
        Ok(as_json.into_bytes())
    }
}

pub trait FromJsonPayloadExt: Sized {
    fn from_json_payload(payload: &[u8]) -> Result<Self, anyhow::Error>;
}
impl<T> FromJsonPayloadExt for T
where
    T: for<'de> Deserialize<'de>,
{
    fn from_json_payload(payload: &[u8]) -> Result<Self, anyhow::Error> {
        let payload_str = std::str::from_utf8(payload).map_err(anyhow::Error::from)?;
        Ok(serde_json::from_str(payload_str)?)
    }
}

pub struct OrchestratorContext {
    actions: Sender<String>,
    tasks: Sender<String>,
}

impl OrchestratorContext {
    pub(crate) fn new(instance_id: String) -> Self {
        let (actions_tx, actions_rx) = std::sync::mpsc::channel();
        let (tasks_tx, tasks_rx) = std::sync::mpsc::channel();
        OrchestratorContext {
            actions: actions_tx,
            tasks: tasks_tx,
        }
    }
}

/// Orchestrator result
pub type OrchestratorResult<T> = Result<OrchestratorResultValue<T>, anyhow::Error>;

/// Orchestrator result value
#[derive(Debug)]
pub enum OrchestratorResultValue<T> {
    ContinueAsNew,
    Output(T),
}

type OrchestratorFn =
    dyn Fn(OrchestratorContext) -> BoxFuture<'static, OrchestratorResult<Vec<u8>>> + Send + 'static;

pub struct OrchestratorFunction {
    orchestrator_fn: Box<OrchestratorFn>,
}

impl<F, Fut, O> From<F> for OrchestratorFunction
where
    F: Fn(OrchestratorContext) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = OrchestratorResult<O>> + Send + 'static,
    O: Serialize,
{
    fn from(orchestration_fn: F) -> Self {
        Self::new(orchestration_fn)
    }
}

impl OrchestratorFunction {
    pub fn new<F, Fut, O>(f: F) -> Self
    where
        F: Fn(OrchestratorContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = OrchestratorResult<O>> + Send + 'static,
        O: Serialize,
    {
        Self {
            orchestrator_fn: Box::new(move |ctx: OrchestratorContext| {
                f(ctx)
                    .map(|r| {
                        r.and_then(|r| {
                            Ok(match r {
                                OrchestratorResultValue::ContinueAsNew => {
                                    OrchestratorResultValue::ContinueAsNew
                                }
                                OrchestratorResultValue::Output(o) => {
                                    OrchestratorResultValue::Output(o.as_json_payload()?)
                                }
                            })
                        })
                    })
                    .boxed()
            }),
        }
    }
}

impl OrchestratorFunction {
    pub(crate) fn call(
        &self,
        ctx: OrchestratorContext,
    ) -> BoxFuture<'static, OrchestratorResult<Vec<u8>>> {
        (self.orchestrator_fn)(ctx)
    }
}

pub struct ActivityContext {
    pub orchestration_id: String,
    pub task_id: u32,
}

/// Activity result
pub type ActivityResult<T> = Result<ActivityResultValue<T>, anyhow::Error>;

/// Activity result value
#[derive(Debug)]
pub enum ActivityResultValue<T> {
    Failure,
    Output(T),
}

type ActivityFn = dyn Fn(ActivityContext) -> BoxFuture<'static, Result<ActivityResultValue<Vec<u8>>, anyhow::Error>>
    + Send
    + 'static;

pub struct ActivityFunction {
    activity_fn: Box<ActivityFn>,
}
