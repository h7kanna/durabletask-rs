// API portions are similar to temporal-sdk for now.
// Errors are not handled anyhow everywhere to speed up prototyping
use crate::internal::new_schedule_task_action;
use crate::task::CompletableTask;
use durabletask_proto::OrchestratorAction;
use futures_util::future::BoxFuture;
use futures_util::FutureExt;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::future::Future;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use tokio::sync::oneshot;

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

/// Orchestrator result
pub type OrchestratorResult<T> = Result<OrchestratorResultValue<T>, anyhow::Error>;

/// Orchestrator result value
#[derive(Debug)]
pub enum OrchestratorResultValue<T> {
    ContinueAsNew,
    Output(T),
}

type OrchestratorFn = dyn Fn(OrchestratorContext) -> BoxFuture<'static, OrchestratorResult<Vec<u8>>>
    + Send
    + Sync
    + 'static;

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

pub struct ActivityOptions {
    pub activity_type: String,
}

pub struct ActivityContext {
    pub orchestration_id: String,
    pub task_id: i32,
}

/// Activity result
pub type ActivityResult<T> = Result<T, anyhow::Error>;

type ActivityFn = Arc<
    dyn Fn(ActivityContext, Vec<u8>) -> BoxFuture<'static, Result<Vec<u8>, anyhow::Error>>
        + Send
        + Sync,
>;

pub struct ActivityFunction {
    pub(crate) activity_fn: ActivityFn,
}

pub trait IntoActivityFunc<Args, Res, Out> {
    fn into_activity_fn(self) -> ActivityFn;
}

impl<A, Rf, O, F> IntoActivityFunc<A, Rf, O> for F
where
    F: (Fn(ActivityContext, A) -> Rf) + Sync + Send + 'static,
    A: FromJsonPayloadExt + Send,
    Rf: Future<Output = Result<O, anyhow::Error>> + Send + 'static,
    O: AsJsonPayloadExt,
{
    fn into_activity_fn(self) -> ActivityFn {
        let wrapper = move |ctx: ActivityContext, input: Vec<u8>| match A::from_json_payload(&input)
        {
            Ok(deser) => self(ctx, deser)
                .map(|r| {
                    r.and_then(|r| match r.as_json_payload() {
                        Ok(v) => Ok(v),
                        Err(e) => Err(e.into()),
                    })
                })
                .boxed(),
            Err(e) => async move { Err(e.into()) }.boxed(),
        };
        Arc::new(wrapper)
    }
}

/// Trait to represent an async function with 2 arguments
pub trait AsyncFn<Arg0, Arg1>: Fn(Arg0, Arg1) -> Self::OutputFuture {
    /// Output type of the async function which implements serde traits
    type Output;
    /// Future of the output
    type OutputFuture: Future<Output = <Self as AsyncFn<Arg0, Arg1>>::Output> + Send + 'static;
}

impl<F: ?Sized, Fut, Arg0, Arg1> AsyncFn<Arg0, Arg1> for F
where
    F: Fn(Arg0, Arg1) -> Fut,
    Fut: Future + Send + 'static,
{
    type Output = Fut::Output;
    type OutputFuture = Fut;
}

pub struct OrchestratorContext {
    sequence_number: Arc<RwLock<i32>>,
    actions_tx: Sender<(i32, OrchestratorAction)>,
    tasks_tx: Sender<(i32, oneshot::Sender<String>)>,
}

impl OrchestratorContext {
    pub(crate) fn new(
        instance_id: String,
    ) -> (
        Self,
        Receiver<(i32, OrchestratorAction)>,
        Receiver<(i32, oneshot::Sender<String>)>,
    ) {
        let (actions_tx, actions_rx) = std::sync::mpsc::channel();
        let (tasks_tx, tasks_rx) = std::sync::mpsc::channel();
        (
            OrchestratorContext {
                sequence_number: Arc::new(RwLock::new(0)),
                actions_tx,
                tasks_tx,
            },
            actions_rx,
            tasks_rx,
        )
    }

    fn next_sequence_number(&self) -> i32 {
        let mut sequence_number = self.sequence_number.write();
        *sequence_number += 1;
        *sequence_number
    }

    pub fn instance_id(&self) -> String {
        "".to_string()
    }

    pub fn create_timer(&self) {}

    pub async fn call_activity<A, F, R>(
        &self,
        options: ActivityOptions,
        _f: F,
        a: A,
    ) -> Result<R, anyhow::Error>
    where
        F: AsyncFn<ActivityContext, A, Output = ActivityResult<R>> + Send + 'static,
        A: AsJsonPayloadExt + Debug,
        R: FromJsonPayloadExt + Debug,
    {
        let input = A::as_json_payload(&a).expect("input serialization failed");
        let activity_type = if options.activity_type.is_empty() {
            std::any::type_name::<F>().to_string()
        } else {
            options.activity_type
        };
        let options = ActivityOptions {
            activity_type,
            ..options
        };
        let activity_result = self.schedule_activity(&options.activity_type, None).await;

        Ok(R::from_json_payload(&activity_result).expect("output deserialization failed"))
    }

    pub fn schedule_activity(&self, name: &str, input: Option<&str>) -> CompletableTask {
        let id = self.next_sequence_number();
        let action = new_schedule_task_action(id, name, input);
        self.actions_tx.send((id, action)).expect("cannot happen");
        let (task, unblock) = CompletableTask::new();
        self.tasks_tx.send((id, unblock)).expect("cannot happen");
        println!("Call activity task {}", id);
        task
    }

    pub fn call_sub_orchestrator(&self) -> impl Future<Output = ()> {
        std::future::pending::<()>()
    }

    pub fn await_signal_event(&self) -> impl Future<Output = ()> {
        std::future::pending::<()>()
    }

    pub fn continue_as_new(&self) -> impl Future<Output = ()> {
        std::future::pending::<()>()
    }
}