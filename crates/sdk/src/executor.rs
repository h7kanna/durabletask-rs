use crate::registry::Registry;
use crate::types::{OrchestratorContext, OrchestratorResult};
use durabletask_proto::history_event::EventType;
use durabletask_proto::{HistoryEvent, OrchestratorAction};
use futures_util::future::BoxFuture;
use futures_util::FutureExt;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::oneshot;

pub struct OrchestrationExecutor {
    registry: Arc<Registry>,
}

impl OrchestrationExecutor {
    pub fn new(registry: Arc<Registry>) -> Self {
        Self { registry }
    }
    pub fn execute(
        &self,
        instance_id: String,
        old_events: Vec<HistoryEvent>,
        new_events: Vec<HistoryEvent>,
    ) -> impl Future<Output = Result<Vec<OrchestratorAction>, anyhow::Error>> {
        OrchestrationExecutorFuture {
            instance_id,
            registry: self.registry.clone(),
            old_events: Arc::new(old_events),
            new_events: Arc::new(new_events),
        }
    }
}

#[derive(Default)]
pub struct OrchestrationExecutorContext {
    is_replaying: bool,
    pending_actions: HashMap<i32, OrchestratorAction>,
    pending_tasks: HashMap<i32, oneshot::Sender<String>>,
    orchestrator_fn: Option<BoxFuture<'static, OrchestratorResult<Vec<u8>>>>,
    action_rx: Option<Receiver<(i32, OrchestratorAction)>>,
    task_rx: Option<Receiver<(i32, oneshot::Sender<String>)>>,
}

pub struct OrchestrationExecutorFuture {
    instance_id: String,
    registry: Arc<Registry>,
    old_events: Arc<Vec<HistoryEvent>>,
    new_events: Arc<Vec<HistoryEvent>>,
}

impl OrchestrationExecutorFuture {
    fn process_event(
        &mut self,
        ctx: &mut OrchestrationExecutorContext,
        cx: &mut Context<'_>,
        event: &HistoryEvent,
    ) {
        if let Some(event_type) = &event.event_type {
            match event_type {
                EventType::OrchestratorStarted(event) => {}
                EventType::ExecutionStarted(event) => {
                    let orchestrator_fn = self.registry.get_orchestrator(&event.name);
                    if let Some(orchestrator_fn) = orchestrator_fn {
                        let (octx, actions_rx, tasks_rx) =
                            OrchestratorContext::new(self.instance_id.clone());
                        let mut orchestrator_fn = orchestrator_fn.call(octx);
                        match orchestrator_fn.poll_unpin(cx) {
                            Poll::Ready(_) => {
                                println!("Orchestrator run");
                            }
                            Poll::Pending => {
                                println!("Orchestrator initialized");
                                ctx.orchestrator_fn = Some(orchestrator_fn);
                                ctx.action_rx = Some(actions_rx);
                                ctx.task_rx = Some(tasks_rx);
                            }
                        }
                    } else {
                        // Error not found
                    }
                }
                EventType::ExecutionCompleted(event) => {
                    if let Some(orchestrator_fn) = &mut ctx.orchestrator_fn {
                        match orchestrator_fn.poll_unpin(cx) {
                            Poll::Ready(_) => {}
                            Poll::Pending => {}
                        }
                    }
                }
                EventType::ExecutionTerminated(_) => {}
                EventType::TaskScheduled(task_event) => {
                    let task_id = event.event_id;
                    if let Some(action) = ctx.pending_actions.remove(&task_id) {
                        println!("action {:?}", action);
                    } else {
                        println!("Non determinism");
                    }
                }
                EventType::TaskCompleted(task_completed_event) => {
                    let task_id = task_completed_event.task_scheduled_id;
                    if let Some(task) = ctx.pending_tasks.remove(&task_id) {
                        println!("task {:?}", task);
                        if let Some(result) = &task_completed_event.result {
                            // How to avoid clone here?
                            task.send(result.clone())
                                .expect("failed to unblock activity")
                        }
                    } else {
                        println!("Non determinism");
                    }
                }
                EventType::TaskFailed(task_failed_event) => {
                    let task_id = task_failed_event.task_scheduled_id;
                    if let Some(task) = ctx.pending_tasks.remove(&task_id) {
                        println!("task {:?}", task);
                        if let Some(result) = &task_failed_event.failure_details {
                            // How to avoid clone here?
                            task.send(result.error_message.clone())
                                .expect("failed to unblock activity")
                        }
                    } else {
                        println!("Non determinism");
                    }
                }
                EventType::SubOrchestrationInstanceCreated(_) => {}
                EventType::SubOrchestrationInstanceCompleted(_) => {}
                EventType::SubOrchestrationInstanceFailed(_) => {}
                EventType::TimerCreated(_) => {}
                EventType::TimerFired(_) => {}
                EventType::EventSent(event) => {}
                EventType::EventRaised(event) => {}
                EventType::GenericEvent(event) => {}
                EventType::HistoryState(event) => {}
                EventType::ContinueAsNew(_) => {}
                EventType::ExecutionSuspended(_) => {}
                EventType::ExecutionResumed(_) => {}
                EventType::OrchestratorCompleted(event) => {}
            }
        }
    }
    fn process_events(
        &mut self,
        ctx: &mut OrchestrationExecutorContext,
        cx: &mut Context<'_>,
        events: Arc<Vec<HistoryEvent>>,
    ) {
        for event in events.as_ref() {
            self.process_event(ctx, cx, event);
            if let Some(action_rx) = &ctx.action_rx {
                if let Ok((sequence_num, action)) = action_rx.try_recv() {
                    ctx.pending_actions.insert(sequence_num, action);
                }
            }
            if let Some(task_rx) = &ctx.task_rx {
                if let Ok((sequence_num, action)) = task_rx.try_recv() {
                    ctx.pending_tasks.insert(sequence_num, action);
                }
            }
        }
    }
}

impl Future for OrchestrationExecutorFuture {
    type Output = Result<Vec<OrchestratorAction>, anyhow::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let is_suspended = false;
        let suspended_events: Vec<HistoryEvent> = vec![];
        let mut ctx = OrchestrationExecutorContext::default();
        ctx.is_replaying = true;
        let old_events = self.old_events.clone();
        self.process_events(&mut ctx, cx, old_events);
        ctx.is_replaying = false;
        let new_events = self.new_events.clone();
        self.process_events(&mut ctx, cx, new_events);
        Poll::Ready(Ok(vec![]))
    }
}

pub struct ActivityExecutor {
    registry: Arc<Registry>,
}

impl ActivityExecutor {
    pub fn new(registry: Arc<Registry>) -> Self {
        Self { registry }
    }
    pub async fn execute(
        &self,
        instance_id: String,
        name: String,
        task_id: i32,
        input: Option<String>,
    ) -> Result<Option<String>, anyhow::Error> {
        if let Some(activity_fn) = self.registry.get_activity(&name) {
            Ok(Some("testing".to_string()))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::internal::{
        new_execution_started_event, new_orchestrator_started_event, new_task_scheduled_event,
    };
    use crate::types::{
        ActivityContext, ActivityOptions, ActivityResult, OrchestratorContext,
        OrchestratorFunction, OrchestratorResult, OrchestratorResultValue,
    };
    use futures_util::FutureExt;

    async fn test_activity(ctx: ActivityContext, input: String) -> ActivityResult<String> {
        println!("Activity done");
        Ok("hello".to_string())
    }

    async fn test_orchestration(ctx: OrchestratorContext) -> OrchestratorResult<String> {
        println!("Hello");
        let _ = ctx
            .call_activity(
                ActivityOptions {
                    activity_type: "test".to_string(),
                },
                test_activity,
                "input".into(),
            )
            .await;
        println!("Activity done");
        Ok(OrchestratorResultValue::Output("hello".to_string()))
    }

    #[tokio::test]
    async fn test_context() {
        let fn_ = OrchestratorFunction::new(move |ctx: OrchestratorContext| {
            test_orchestration(ctx).boxed()
        });
        let fn2_ = OrchestratorFunction::new(move |ctx: OrchestratorContext| async move {
            Ok(OrchestratorResultValue::Output("hello".to_string()))
        });

        let mut registry = Registry::default();
        registry.add_orchestrator("test".into(), fn_);
        registry.add_orchestrator("test2".into(), fn2_);

        let executor = OrchestrationExecutor::new(Arc::new(registry));
        let actions = executor
            .execute(
                "test".to_string(),
                vec![],
                vec![
                    new_orchestrator_started_event(),
                    new_execution_started_event("test", "test", None, None, None, None),
                    new_task_scheduled_event(1, "test", None, None, None),
                ],
            )
            .await;
        assert_eq!(actions.is_ok(), true);
        let actions = actions.unwrap();
        assert_eq!(actions.len(), 0);
    }
}
