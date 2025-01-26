use crate::environment::Environment;
use crate::internal::{new_complete_orchestration_action, new_terminate_orchestration_action};
use crate::registry::Registry;
use crate::types::{
    ActivityContext, OrchestratorContext, OrchestratorResult, OrchestratorResultValue,
};
use anyhow::anyhow;
use durabletask_proto::history_event::EventType;
use durabletask_proto::{HistoryEvent, OrchestrationStatus, OrchestratorAction};
use futures_util::future::BoxFuture;
use futures_util::FutureExt;
use parking_lot::RwLock;
use prost_wkt_types::Timestamp;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::oneshot;
use tracing::debug;

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
    is_complete: bool,
    sequence_number: i32,
    current_time_utc: Timestamp,
    pending_actions: HashMap<i32, OrchestratorAction>,
    pending_tasks: HashMap<i32, oneshot::Sender<Option<String>>>,
    received_events: Arc<RwLock<HashMap<String, VecDeque<Option<String>>>>>,
    pending_events: HashMap<String, VecDeque<oneshot::Sender<Option<String>>>>,
    orchestrator_fn: Option<BoxFuture<'static, OrchestratorResult<Vec<u8>>>>,
    action_rx: Option<Receiver<(i32, OrchestratorAction)>>,
    task_rx: Option<Receiver<(i32, oneshot::Sender<Option<String>>)>>,
    events_rx: Option<Receiver<(String, oneshot::Sender<Option<String>>)>>,
    completion_status: OrchestrationStatus,
}

impl OrchestrationExecutorContext {
    fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
    fn next_sequence_number(&mut self) -> i32 {
        self.sequence_number += 1;
        self.sequence_number
    }
    fn get_actions(&self) -> Vec<OrchestratorAction> {
        self.pending_actions
            .values()
            .into_iter()
            .map(|action| action.clone())
            .collect()
    }
    fn set_complete(&mut self, status: OrchestrationStatus) {
        if self.is_complete {
            return;
        }
        self.completion_status = status;
    }
}

pub struct OrchestrationExecutorFuture {
    instance_id: String,
    registry: Arc<Registry>,
    old_events: Arc<Vec<HistoryEvent>>,
    new_events: Arc<Vec<HistoryEvent>>,
}

impl OrchestrationExecutorFuture {
    fn poll(
        &mut self,
        ctx: &mut OrchestrationExecutorContext,
        cx: &mut Context<'_>,
    ) -> Result<(), anyhow::Error> {
        if ctx.is_complete {
            return Ok(());
        }
        if let Some(orchestrator_fn) = &mut ctx.orchestrator_fn {
            match orchestrator_fn.poll_unpin(cx) {
                Poll::Ready(result) => {
                    let task_id = ctx.next_sequence_number();
                    let action = match result {
                        Ok(result) => match result {
                            OrchestratorResultValue::ContinueAsNew => {
                                new_complete_orchestration_action(
                                    task_id,
                                    OrchestrationStatus::ContinuedAsNew,
                                    None,
                                    &vec![],
                                    None,
                                )
                            }
                            OrchestratorResultValue::Output(output) => {
                                new_complete_orchestration_action(
                                    task_id,
                                    OrchestrationStatus::Completed,
                                    Some(String::from_utf8(output).unwrap().as_str()),
                                    &vec![],
                                    None,
                                )
                            }
                        },
                        Err(failure) => new_complete_orchestration_action(
                            task_id,
                            OrchestrationStatus::Failed,
                            None,
                            &vec![],
                            None,
                        ),
                    };
                    ctx.pending_actions.insert(task_id, action);
                    Ok(())
                }
                Poll::Pending => Ok(()),
            }
        } else {
            Err(anyhow!("Polled without orchestration future"))
        }
    }

    fn process_event(
        &mut self,
        ctx: &mut OrchestrationExecutorContext,
        cx: &mut Context<'_>,
        event: &HistoryEvent,
    ) {
        if let Some(event_type) = &event.event_type {
            match event_type {
                EventType::OrchestratorStarted(started_event) => {
                    if let Some(timestamp) = event.timestamp {
                        ctx.current_time_utc = timestamp;
                    }
                }
                EventType::ExecutionStarted(event) => {
                    let orchestrator_fn = self.registry.get_orchestrator(&event.name);
                    if let Some(orchestrator_fn) = orchestrator_fn {
                        let (octx, actions_rx, tasks_rx, events_rx) = OrchestratorContext::new(
                            self.instance_id.clone(),
                            event.input.clone(),
                            ctx.received_events.clone(),
                        );
                        let mut orchestrator_fn = orchestrator_fn.call(octx);
                        match orchestrator_fn.poll_unpin(cx) {
                            Poll::Ready(_) => {
                                debug!("Orchestrator run");
                            }
                            Poll::Pending => {
                                debug!("Orchestrator initialized");
                                ctx.orchestrator_fn = Some(orchestrator_fn);
                                ctx.action_rx = Some(actions_rx);
                                ctx.task_rx = Some(tasks_rx);
                                ctx.events_rx = Some(events_rx);
                            }
                        }
                    } else {
                        // Error not found
                    }
                }
                EventType::ExecutionCompleted(event) => {
                    if let Some(orchestrator_fn) = &mut ctx.orchestrator_fn {
                        match orchestrator_fn.poll_unpin(cx) {
                            Poll::Ready(output) => {}
                            Poll::Pending => {}
                        }
                    }
                }
                EventType::ExecutionTerminated(terminated_event) => {
                    ctx.set_complete(OrchestrationStatus::Terminated);
                    let task_id = ctx.next_sequence_number();
                    /*
                    ctx.pending_actions.insert(
                        task_id,
                        new_terminate_orchestration_action(
                            task_id,
                            &self.instance_id,
                            terminated_event.recurse,
                            terminated_event.input.as_ref().map(|r| r.as_str()),
                        ),
                    );*/
                    ctx.pending_actions.insert(
                        task_id,
                        new_complete_orchestration_action(
                            task_id,
                            OrchestrationStatus::Terminated,
                            None,
                            &vec![],
                            None,
                        ),
                    );
                }
                EventType::TaskScheduled(task_event) => {
                    let task_id = event.event_id;
                    if let Some(action) = ctx.pending_actions.remove(&task_id) {
                        debug!("TaskScheduled: action {:?}", action);
                    } else {
                        debug!("Non determinism");
                    }
                }
                EventType::TaskCompleted(task_completed_event) => {
                    let task_id = task_completed_event.task_scheduled_id;
                    if let Some(task) = ctx.pending_tasks.remove(&task_id) {
                        debug!("TaskCompleted task completed {:?}", task);
                        if let Some(result) = &task_completed_event.result {
                            // How to avoid clone here?
                            task.send(Some(result.clone()))
                                .expect("failed to unblock activity");
                        }
                    } else {
                        debug!("Non determinism");
                    }
                    self.poll(ctx, cx).unwrap()
                }
                EventType::TaskFailed(task_failed_event) => {
                    let task_id = task_failed_event.task_scheduled_id;
                    if let Some(task) = ctx.pending_tasks.remove(&task_id) {
                        debug!("task {:?}", task);
                        if let Some(result) = &task_failed_event.failure_details {
                            // How to avoid clone here?
                            task.send(Some(result.error_message.clone()))
                                .expect("failed to unblock activity")
                        }
                    } else {
                        debug!("Non determinism");
                    }
                }
                EventType::SubOrchestrationInstanceCreated(_) => {
                    let task_id = event.event_id;
                    if let Some(action) = ctx.pending_actions.remove(&task_id) {
                        debug!("SubOrchestrationInstanceCreated: action {:?}", action);
                    } else {
                        debug!("Non determinism");
                    }
                }
                EventType::SubOrchestrationInstanceCompleted(task_completed_event) => {
                    let task_id = task_completed_event.task_scheduled_id;
                    if let Some(task) = ctx.pending_tasks.remove(&task_id) {
                        debug!(
                            "SubOrchestrationInstanceCompleted task completed {:?}",
                            task
                        );
                        if let Some(result) = &task_completed_event.result {
                            // How to avoid clone here?
                            task.send(Some(result.clone()))
                                .expect("failed to unblock sub orchestrator");
                        }
                    } else {
                        debug!("Non determinism");
                    }
                    self.poll(ctx, cx).unwrap()
                }
                EventType::SubOrchestrationInstanceFailed(task_failed_event) => {
                    let task_id = task_failed_event.task_scheduled_id;
                    if let Some(task) = ctx.pending_tasks.remove(&task_id) {
                        debug!("task {:?}", task);
                        if let Some(result) = &task_failed_event.failure_details {
                            // How to avoid clone here?
                            task.send(Some(result.error_message.clone()))
                                .expect("failed to unblock activity")
                        }
                    } else {
                        debug!("Non determinism");
                    }
                    self.poll(ctx, cx).unwrap()
                }
                EventType::TimerCreated(timer_created_event) => {
                    let task_id = event.event_id;
                    if let Some(action) = ctx.pending_actions.remove(&task_id) {
                        debug!("TimerCreated: action {:?}", action);
                    } else {
                        debug!("Non determinism");
                    }
                }
                EventType::TimerFired(timer_fired_event) => {
                    let task_id = timer_fired_event.timer_id;
                    if let Some(task) = ctx.pending_tasks.remove(&task_id) {
                        debug!("TimerFired task completed {:?}", task);
                        if let Some(result) = &timer_fired_event.fire_at {
                            // How to avoid clone here?
                            task.send(Some("".to_string()))
                                .expect("failed to unblock timer");
                        }
                    } else {
                        debug!("Non determinism");
                    }
                    self.poll(ctx, cx).unwrap()
                }
                EventType::EventSent(event) => {}
                EventType::EventRaised(event) => {
                    let name = event.name.to_lowercase();
                    let mut remove = false;
                    if let Some(pending_tasks) = ctx.pending_events.get_mut(&name) {
                        let task = if pending_tasks.len() > 1 {
                            pending_tasks.pop_front()
                        } else {
                            remove = true;
                            pending_tasks.pop_front()
                        };
                        if remove {
                            ctx.pending_events.remove(&name);
                        }
                        task.unwrap().send(event.input.clone()).unwrap();
                        self.poll(ctx, cx).unwrap()
                    } else {
                        let mut lock = ctx.received_events.write();
                        let mut received_events = if let Some(received_events) = lock.get_mut(&name)
                        {
                            received_events
                        } else {
                            lock.insert(name.clone(), VecDeque::new());
                            lock.get_mut(&name).unwrap()
                        };
                        received_events.push_back(event.input.clone())
                    }
                }
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
                while let Ok((sequence_number, action)) = action_rx.try_recv() {
                    ctx.pending_actions.insert(sequence_number, action);
                    // TODO: Remove this after cleaning up complete task sender side
                    if sequence_number > 0 {
                        ctx.sequence_number = sequence_number;
                    }
                }
            }
            if let Some(task_rx) = &ctx.task_rx {
                while let Ok((sequence_number, task)) = task_rx.try_recv() {
                    ctx.pending_tasks.insert(sequence_number, task);
                }
            }
            if let Some(events_rx) = &ctx.events_rx {
                while let Ok((event, input)) = events_rx.try_recv() {
                    let mut pending_events =
                        if let Some(pending_events) = ctx.pending_events.get_mut(&event) {
                            pending_events
                        } else {
                            ctx.pending_events.insert(event.clone(), VecDeque::new());
                            ctx.pending_events.get_mut(&event).unwrap()
                        };
                    pending_events.push_back(input)
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
        let mut ctx = OrchestrationExecutorContext::new();
        ctx.is_replaying = true;
        tracing::Span::current().record("replaying", true);
        let old_events = self.old_events.clone();
        self.process_events(&mut ctx, cx, old_events);
        ctx.is_replaying = false;
        tracing::Span::current().record("replaying", false);
        let new_events = self.new_events.clone();
        self.process_events(&mut ctx, cx, new_events);
        Poll::Ready(Ok(ctx.get_actions()))
    }
}

pub struct ActivityExecutor {
    registry: Arc<Registry>,
    environment: Arc<Environment>,
}

impl ActivityExecutor {
    pub fn new(registry: Arc<Registry>, environment: Arc<Environment>) -> Self {
        Self {
            registry,
            environment,
        }
    }
    pub async fn execute(
        &self,
        instance_id: String,
        name: String,
        task_id: i32,
        input: Option<String>,
    ) -> Result<Option<String>, anyhow::Error> {
        if let Some(activity_fn) = self.registry.get_activity(&name) {
            debug!("Executing Activity name: {:?}, task: {}", name, task_id);
            let actx = ActivityContext::new(instance_id, task_id, self.environment.clone());
            let output = activity_fn.call(actx, input).await?;
            Ok(Some(String::from_utf8(output)?))
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
        debug!("Activity test_activity done");
        Ok("hello".to_string())
    }

    async fn test_orchestration(ctx: OrchestratorContext) -> OrchestratorResult<String> {
        debug!("Hello test_orchestration");
        let _ = ctx
            .call_activity(
                test_activity,
                ActivityOptions {
                    activity_type: "test".to_string(),
                },
                "input".into(),
            )
            .await;
        debug!("test_orchestration ------->>> Activity done");
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
