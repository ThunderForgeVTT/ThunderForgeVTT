//! One span and one histogram point per GraphQL operation (R20, FR-011).
//!
//! The labels are bounded by the schema, never by the client:
//!
//! - `root_field` is the operation's first root field when the schema has it,
//!   and `unknown` otherwise; a second root field sets `root_fields=multiple`
//!   on the span, and changes no label;
//! - `outcome` is `ok`, `refused` when every error is an authorisation code,
//!   or `error`;
//! - `code` is an error's `extensions.code`, or `internal` when it has none
//!   or when it does not look like one of ours.
//!
//! The client's operation name goes on the span only. The anonymous span
//! processor drops it, and no instrument ever sees it.
//!
//! A query or a mutation is timed from its request to its response, parse
//! and validation included. A subscription's span covers only its setup,
//! because a stream that lives for an hour is not an hour-long operation.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextParseQuery, NextPrepareRequest, NextRequest,
    NextSubscribe,
};
use async_graphql::parser::types::{
    DocumentOperations, ExecutableDocument, OperationType, Selection, SelectionSet,
};
use async_graphql::{Request, Response, ServerError, ServerResult, Value, Variables};
use futures_util::stream::BoxStream;
use opentelemetry::KeyValue;
use tracing::Instrument;
use tracing::field::Empty;

use super::instruments::{Recorders, recorders};

/// Error codes that mean "not allowed", not "went wrong".
pub const REFUSED_CODES: &[&str] = &["FORBIDDEN", "UNAUTHENTICATED", "NOT_IN_DEMO"];

/// Registered at `Schema::build`. Each request gets its own instance.
#[derive(Clone, Copy)]
pub struct GraphQLTelemetry {
    recorders: Option<&'static Recorders>,
}

impl GraphQLTelemetry {
    /// Records into the installed instruments, if any.
    pub fn installed() -> Self {
        Self {
            recorders: recorders(),
        }
    }

    /// Records into `recorders`; for tests.
    pub fn with(recorders: &'static Recorders) -> Self {
        Self {
            recorders: Some(recorders),
        }
    }
}

impl ExtensionFactory for GraphQLTelemetry {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(PerRequest {
            recorders: self.recorders,
            seen: Mutex::default(),
            subscription: Mutex::default(),
        })
    }
}

/// What parsing told us about the operation.
#[derive(Debug, Clone, Default)]
struct Seen {
    operation_name: Option<String>,
    operation_type: Option<&'static str>,
    root_field: Option<String>,
    multiple: bool,
}

struct PerRequest {
    recorders: Option<&'static Recorders>,
    seen: Mutex<Seen>,
    /// A subscription's span, open from `subscribe` until its query parses.
    subscription: Mutex<Option<(tracing::Span, Instant)>>,
}

impl PerRequest {
    fn seen(&self) -> Seen {
        self.seen.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Closes a subscription's setup span, once.
    fn finish_subscription(&self) {
        let open = self.subscription.lock().ok().and_then(|mut s| s.take());
        if let Some((span, started)) = open {
            let mut seen = self.seen();
            seen.operation_type.get_or_insert("subscription");
            finish(self.recorders, &span, &seen, &[], started);
        }
    }
}

impl Drop for PerRequest {
    /// A subscription whose query never parsed still counts, as `unknown`.
    fn drop(&mut self) {
        self.finish_subscription();
    }
}

fn type_name(ty: OperationType) -> &'static str {
    match ty {
        OperationType::Query => "query",
        OperationType::Mutation => "mutation",
        OperationType::Subscription => "subscription",
    }
}

/// Root field names in order, through inline fragments and fragment spreads.
fn root_fields(doc: &ExecutableDocument, set: &SelectionSet, out: &mut Vec<String>) {
    for item in &set.items {
        match &item.node {
            Selection::Field(f) => out.push(f.node.name.node.to_string()),
            Selection::InlineFragment(i) => root_fields(doc, &i.node.selection_set.node, out),
            Selection::FragmentSpread(s) => {
                if let Some(def) = doc.fragments.get(&s.node.fragment_name.node) {
                    root_fields(doc, &def.node.selection_set.node, out);
                }
            }
        }
    }
}

/// `field` when the schema's root for `ty` has it, else `unknown`.
fn bounded_field(ctx: &ExtensionContext<'_>, ty: OperationType, field: &str) -> String {
    let registry = &ctx.schema_env.registry;
    let root = match ty {
        OperationType::Query => Some(&registry.query_type),
        OperationType::Mutation => registry.mutation_type.as_ref(),
        OperationType::Subscription => registry.subscription_type.as_ref(),
    };
    let known = root
        .and_then(|name| registry.types.get(name))
        .is_some_and(|t| t.field_by_name(field).is_some());
    if known {
        field.to_string()
    } else {
        "unknown".to_string()
    }
}

/// An error's `extensions.code`, or `internal`. A code that is not shaped
/// like ours (upper snake case, short) is `internal` too, so no message can
/// become a label.
pub fn error_code(error: &ServerError) -> String {
    let code = error.extensions.as_ref().and_then(|e| e.get("code"));
    let text = match code {
        Some(Value::String(s)) => s.as_str(),
        Some(Value::Enum(n)) => n.as_str(),
        _ => return "internal".to_string(),
    };
    let ours = !text.is_empty()
        && text.len() <= 48
        && text
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit());
    if ours {
        text.to_string()
    } else {
        "internal".to_string()
    }
}

/// `ok`, `refused` or `error`, from the codes of a response's errors.
pub fn outcome(codes: &[String]) -> &'static str {
    if codes.is_empty() {
        "ok"
    } else if codes.iter().all(|c| REFUSED_CODES.contains(&c.as_str())) {
        "refused"
    } else {
        "error"
    }
}

fn operation_span() -> tracing::Span {
    tracing::info_span!(
        target: super::SPAN_TARGET,
        "graphql.operation",
        otel.name = Empty,
        graphql.operation.type = Empty,
        graphql.root_field = Empty,
        graphql.operation.name = Empty,
        graphql.error.codes = Empty,
        root_fields = Empty,
        outcome = Empty,
    )
}

/// Fills the span, and records the histogram point and the errors.
fn finish(
    recorders: Option<&Recorders>,
    span: &tracing::Span,
    seen: &Seen,
    codes: &[String],
    started: Instant,
) {
    // A request that never parsed has no type; it is counted as a query
    // against `unknown`, which is what it tried to be.
    let ty = seen.operation_type.unwrap_or("query");
    let root = seen.root_field.as_deref().unwrap_or("unknown");
    let outcome = outcome(codes);
    let name = format!("graphql.{ty} {root}");
    span.record("otel.name", name.as_str());
    // The OTel span started when the request first entered it, so the
    // recorded name no longer reaches it; rename it directly.
    span.in_scope(|| opentelemetry::trace::get_active_span(|s| s.update_name(name)));
    span.record("graphql.operation.type", ty);
    span.record("graphql.root_field", root);
    if let Some(name) = &seen.operation_name {
        span.record("graphql.operation.name", name.as_str());
    }
    if seen.multiple {
        span.record("root_fields", "multiple");
    }
    if !codes.is_empty() {
        span.record("graphql.error.codes", codes.join(","));
    }
    span.record("outcome", outcome);

    let Some(r) = recorders else { return };
    r.graphql_duration.record(
        started.elapsed().as_secs_f64(),
        &[
            KeyValue::new("operation_type", ty),
            KeyValue::new("root_field", root.to_string()),
            KeyValue::new("outcome", outcome),
        ],
    );
    for code in codes {
        r.graphql_errors.add(
            1,
            &[
                KeyValue::new("root_field", root.to_string()),
                KeyValue::new("code", code.clone()),
            ],
        );
    }
}

#[async_trait::async_trait]
impl Extension for PerRequest {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.operation_name = request.operation_name.clone();
        }
        next.run(ctx, request).await
    }

    async fn parse_query(
        &self,
        ctx: &ExtensionContext<'_>,
        query: &str,
        variables: &Variables,
        next: NextParseQuery<'_>,
    ) -> ServerResult<ExecutableDocument> {
        let doc = next.run(ctx, query, variables).await?;
        let mut seen = self.seen();
        let op = match &doc.operations {
            DocumentOperations::Single(op) => Some(op),
            // A lone named operation needs no `operationName`.
            DocumentOperations::Multiple(ops)
                if ops.len() == 1 && seen.operation_name.is_none() =>
            {
                ops.iter().next().map(|(name, op)| {
                    seen.operation_name = Some(name.to_string());
                    op
                })
            }
            DocumentOperations::Multiple(ops) => seen
                .operation_name
                .as_deref()
                .and_then(|n| ops.iter().find(|(k, _)| k.as_str() == n).map(|(_, v)| v)),
        };
        if let Some(op) = op {
            let mut fields = Vec::new();
            root_fields(&doc, &op.node.selection_set.node, &mut fields);
            seen.operation_type = Some(type_name(op.node.ty));
            seen.multiple = fields.len() > 1;
            seen.root_field = fields.first().map(|f| bounded_field(ctx, op.node.ty, f));
        }
        if let Ok(mut slot) = self.seen.lock() {
            *slot = seen;
        }
        // A subscription's setup ends here; its stream is not the operation.
        self.finish_subscription();
        Ok(doc)
    }

    async fn request(&self, ctx: &ExtensionContext<'_>, next: NextRequest<'_>) -> Response {
        let started = Instant::now();
        let span = operation_span();
        let response = next.run(ctx).instrument(span.clone()).await;
        let codes: Vec<String> = response.errors.iter().map(error_code).collect();
        finish(self.recorders, &span, &self.seen(), &codes, started);
        response
    }

    fn subscribe<'s>(
        &self,
        ctx: &ExtensionContext<'_>,
        stream: BoxStream<'s, Response>,
        next: NextSubscribe<'_>,
    ) -> BoxStream<'s, Response> {
        // The query is parsed on the stream's first poll, so the span stays
        // open until `parse_query` closes it.
        if let Ok(mut slot) = self.subscription.lock() {
            *slot = Some((operation_span(), Instant::now()));
        }
        next.run(ctx, stream)
    }
}

#[cfg(test)]
#[path = "graphql_extension_tests.rs"]
mod tests;
