//! The extension against a small schema, with an in-memory meter and a
//! layer that keeps every span's fields.

use super::*;
use crate::telemetry::instruments::testing::TestMeter;
use async_graphql::{ErrorExtensions, Object, Schema, Subscription};
use futures_util::StreamExt;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;

struct Query;

#[Object]
impl Query {
    async fn hello(&self) -> &str {
        "hi"
    }
    async fn other(&self) -> i32 {
        1
    }
    async fn guarded(&self) -> async_graphql::Result<i32> {
        Err(async_graphql::Error::new("no").extend_with(|_, e| e.set("code", "FORBIDDEN")))
    }
    async fn broken(&self) -> async_graphql::Result<i32> {
        Err(async_graphql::Error::new("boom"))
    }
}

struct Mutation;

#[Object]
impl Mutation {
    async fn do_it(&self) -> bool {
        true
    }
}

struct Subs;

#[Subscription]
impl Subs {
    async fn ticks(&self) -> impl futures_util::Stream<Item = i32> {
        futures_util::stream::iter([1, 2, 3])
    }
}

type Fields = BTreeMap<String, String>;

#[derive(Clone, Default)]
struct Spans(Arc<Mutex<Vec<(String, Fields)>>>);

struct Collect<'a>(&'a mut Fields);

impl Visit for Collect<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

#[derive(Default)]
struct Index(usize);

impl<S: tracing::Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Spans {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut fields = Fields::new();
        attrs.record(&mut Collect(&mut fields));
        let mut all = self.0.lock().unwrap();
        all.push((attrs.metadata().name().to_string(), fields));
        ctx.span(id)
            .unwrap()
            .extensions_mut()
            .insert(Index(all.len() - 1));
    }
    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let span = ctx.span(id).unwrap();
        let ext = span.extensions();
        let i = ext.get::<Index>().unwrap().0;
        values.record(&mut Collect(&mut self.0.lock().unwrap()[i].1));
    }
}

impl Spans {
    fn operations(&self) -> Vec<Fields> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(n, _)| n == "graphql.operation")
            .map(|(_, f)| f.clone())
            .collect()
    }
}

struct Harness {
    meter: TestMeter,
    spans: Spans,
    schema: Schema<Query, Mutation, Subs>,
}

fn harness() -> Harness {
    let meter = TestMeter::new();
    let recorders: &'static Recorders = Box::leak(Box::new(Recorders::new(&meter.meter)));
    let schema = Schema::build(Query, Mutation, Subs)
        .extension(GraphQLTelemetry::with(recorders))
        .finish();
    Harness {
        meter,
        spans: Spans::default(),
        schema,
    }
}

impl Harness {
    async fn run(&self, request: impl Into<Request>) -> Response {
        let subscriber = tracing_subscriber::registry().with(self.spans.clone());
        let _guard = tracing::subscriber::set_default(subscriber);
        self.schema.execute(request).await
    }
}

/// The histogram's points, by their labels.
fn durations(h: &Harness) -> Vec<String> {
    h.meter
        .collect()
        .into_keys()
        .filter(|k| k.starts_with("thunderforge.graphql.operation.duration{"))
        .collect()
}

#[tokio::test]
async fn a_mutation_gives_one_span_and_one_point() {
    let h = harness();
    let r = h.run("mutation { doIt }").await;
    assert!(r.errors.is_empty());
    let ops = h.spans.operations();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0]["otel.name"], "graphql.mutation doIt");
    assert_eq!(ops[0]["outcome"], "ok");
    assert_eq!(
        durations(&h),
        [
            "thunderforge.graphql.operation.duration{operation_type=mutation,outcome=ok,root_field=doIt}"
        ]
    );
}

#[tokio::test]
async fn a_field_not_in_the_schema_is_unknown() {
    let h = harness();
    let r = h.run("{ nope }").await;
    assert!(!r.errors.is_empty());
    assert_eq!(h.spans.operations()[0]["graphql.root_field"], "unknown");
    let seen = h.meter.collect();
    assert_eq!(
        seen["thunderforge.graphql.errors{code=internal,root_field=unknown}"],
        1.0
    );
    assert!(seen.keys().all(|k| !k.contains("nope")), "{seen:?}");
}

#[tokio::test]
async fn two_root_fields_are_labelled_by_the_first_and_marked_multiple() {
    let h = harness();
    h.run("{ hello other }").await;
    let op = &h.spans.operations()[0];
    assert_eq!(op["root_fields"], "multiple");
    assert_eq!(op["graphql.root_field"], "hello");
}

#[tokio::test]
async fn an_authorisation_code_is_refused() {
    let h = harness();
    h.run("{ guarded }").await;
    assert_eq!(h.spans.operations()[0]["outcome"], "refused");
    let seen = h.meter.collect();
    assert_eq!(
        seen["thunderforge.graphql.errors{code=FORBIDDEN,root_field=guarded}"],
        1.0
    );
    assert!(seen.contains_key(
        "thunderforge.graphql.operation.duration{operation_type=query,outcome=refused,root_field=guarded}"
    ));
}

#[tokio::test]
async fn an_error_without_a_code_is_internal() {
    let h = harness();
    h.run("{ broken }").await;
    assert_eq!(h.spans.operations()[0]["outcome"], "error");
    assert_eq!(
        h.meter.collect()["thunderforge.graphql.errors{code=internal,root_field=broken}"],
        1.0
    );
}

#[tokio::test]
async fn the_operation_name_is_on_the_span_and_in_no_label() {
    let h = harness();
    h.run(Request::new("query SecretClientName { hello }").operation_name("SecretClientName"))
        .await;
    assert_eq!(
        h.spans.operations()[0]["graphql.operation.name"],
        "SecretClientName"
    );
    let seen = h.meter.collect();
    assert!(!seen.is_empty());
    assert!(
        seen.keys().all(|k| !k.contains("SecretClientName")),
        "{seen:?}"
    );
}

#[tokio::test]
async fn a_lone_named_operation_needs_no_operation_name() {
    let h = harness();
    h.run(Request::new("mutation Named { doIt }")).await;
    let op = &h.spans.operations()[0];
    assert_eq!(op["graphql.root_field"], "doIt");
    assert_eq!(op["graphql.operation.name"], "Named");
    assert_eq!(op["otel.name"], "graphql.mutation doIt");
}

#[tokio::test]
async fn a_subscription_span_covers_only_its_setup() {
    let h = harness();
    let subscriber = tracing_subscriber::registry().with(h.spans.clone());
    let _guard = tracing::subscriber::set_default(subscriber);
    let mut stream = h.schema.execute_stream("subscription { ticks }");
    let first = stream.next().await.expect("an item");
    assert!(first.errors.is_empty());
    // Closed after setup, while the stream still has items to give.
    let ops = h.spans.operations();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0]["otel.name"], "graphql.subscription ticks");
    assert_eq!(durations(&h).len(), 1);
    let rest: Vec<_> = stream.collect().await;
    assert_eq!(rest.len(), 2);
    assert_eq!(h.spans.operations().len(), 1, "no span per item");
    assert_eq!(durations(&h).len(), 1);
}

#[test]
fn a_message_shaped_code_is_internal() {
    let e = async_graphql::Error::new("x")
        .extend_with(|_, e| e.set("code", "user typed this"))
        .into_server_error(Default::default());
    assert_eq!(error_code(&e), "internal");
}

#[test]
fn outcome_rules() {
    assert_eq!(outcome(&[]), "ok");
    assert_eq!(
        outcome(&["UNAUTHENTICATED".into(), "NOT_IN_DEMO".into()]),
        "refused"
    );
    assert_eq!(outcome(&["FORBIDDEN".into(), "internal".into()]), "error");
}
