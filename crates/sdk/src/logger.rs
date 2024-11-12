use std::fmt::Debug;
use tracing::{
    field::{Field, Visit},
    span::{Attributes, Record},
    Event, Id, Metadata, Subscriber,
};
use tracing_subscriber::{
    layer::{Context, Filter},
    registry::LookupSpan,
    Layer,
};

#[derive(Debug)]
struct ReplayField(bool);

struct ReplayFieldVisitor(bool);

impl Visit for ReplayFieldVisitor {
    fn record_bool(&mut self, field: &Field, value: bool) {
        if field.name().eq("replay") {
            self.0 = value;
        }
    }

    fn record_debug(&mut self, _field: &Field, _value: &dyn Debug) {}
}

pub struct ReplayFilter;

impl ReplayFilter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ReplayFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Subscriber + for<'lookup> LookupSpan<'lookup>> Filter<S> for ReplayFilter {
    fn enabled(&self, _meta: &Metadata<'_>, _cx: &Context<'_, S>) -> bool {
        true
    }

    fn event_enabled(&self, event: &Event<'_>, cx: &Context<'_, S>) -> bool {
        if let Some(scope) = cx.event_scope(event) {
            let span = scope.from_root().next().unwrap();
            let extensions = span.extensions();
            let replay: &ReplayField = extensions.get::<ReplayField>().unwrap();
            !replay.0
        } else {
            true
        }
    }

    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut visitor = ReplayFieldVisitor(false);
        attrs.record(&mut visitor);
        let span = ctx.span(id).unwrap();
        let mut extensions = span.extensions_mut();
        extensions.insert::<ReplayField>(ReplayField(visitor.0));
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let mut visitor = ReplayFieldVisitor(false);
        values.record(&mut visitor);
        let span = ctx.span(id).unwrap();
        let mut extensions = span.extensions_mut();
        extensions.replace::<ReplayField>(ReplayField(visitor.0));
    }
}

impl<S: Subscriber> Layer<S> for ReplayFilter {}

#[cfg(test)]
mod tests {
    #[test]
    fn test_filter() {}
}
