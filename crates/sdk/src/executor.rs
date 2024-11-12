use crate::registry::Registry;
use crate::types::{OrchestratorContext, OrchestratorResult};
use durabletask_proto::history_event::EventType;
use durabletask_proto::{HistoryEvent, OrchestratorAction};
use futures_util::FutureExt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

#[derive(Default)]
pub struct OrchestrationExecutorContext {
    is_replaying: bool,
}

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
            registry: self.registry.clone(),
            instance_id,
            old_events,
            new_events,
        }
    }
}

pub struct OrchestrationExecutorFuture {
    instance_id: String,
    old_events: Vec<HistoryEvent>,
    new_events: Vec<HistoryEvent>,
    registry: Arc<Registry>,
}

impl OrchestrationExecutorFuture {
    fn process_event(&self, cx: &mut Context<'_>, event: &HistoryEvent) {
        if let Some(event) = &event.event_type {
            match event {
                EventType::OrchestratorStarted(event) => {}
                EventType::ExecutionStarted(event) => {
                    let orchestrator_fn = self.registry.get_orchestrator(&event.name);
                    if let Some(orchestrator_fn) = orchestrator_fn {
                        let ctx = OrchestratorContext::new(self.instance_id.clone());
                        let mut fut = orchestrator_fn.call(ctx);
                        match fut.poll_unpin(cx) {
                            Poll::Ready(_) => {
                                println!("Orchestrator run");
                            }
                            Poll::Pending => {}
                        }
                    }
                }
                EventType::ExecutionCompleted(_) => {}
                EventType::ExecutionTerminated(_) => {}
                EventType::TaskScheduled(_) => {}
                EventType::TaskCompleted(_) => {}
                EventType::TaskFailed(_) => {}
                EventType::SubOrchestrationInstanceCreated(_) => {}
                EventType::SubOrchestrationInstanceCompleted(_) => {}
                EventType::SubOrchestrationInstanceFailed(_) => {}
                EventType::TimerCreated(_) => {}
                EventType::TimerFired(_) => {}
                EventType::EventSent(_) => {}
                EventType::EventRaised(_) => {}
                EventType::GenericEvent(_) => {}
                EventType::HistoryState(_) => {}
                EventType::ContinueAsNew(_) => {}
                EventType::ExecutionSuspended(_) => {}
                EventType::ExecutionResumed(_) => {}
                EventType::OrchestratorCompleted(_) => {}
            }
        }
    }
}

impl Future for OrchestrationExecutorFuture {
    type Output = Result<Vec<OrchestratorAction>, anyhow::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let is_suspended = false;
        let suspended_events: Vec<HistoryEvent> = vec![];
        let mut ctx = OrchestrationExecutorContext::default();
        ctx.is_replaying = true;
        for event in &self.old_events {
            self.process_event(cx, event);
        }
        ctx.is_replaying = false;
        for event in &self.new_events {
            self.process_event(cx, event);
        }
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
    pub async fn execute(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::internal::{new_execution_started_event, new_orchestrator_started_event};
    use crate::types::{
        OrchestratorContext, OrchestratorFunction, OrchestratorResult, OrchestratorResultValue,
    };
    use futures_util::FutureExt;

    async fn test_orchestration(ctx: OrchestratorContext) -> OrchestratorResult<String> {
        println!("Hello");
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
                ],
            )
            .await;
        assert_eq!(actions.is_ok(), true);
        let actions = actions.unwrap();
        assert_eq!(actions.len(), 0);
    }
}
