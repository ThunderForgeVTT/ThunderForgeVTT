//! Spec 086 SC-001: Bunyan's stdout stays what it was.
//!
//! The spans opened for OpenTelemetry (`SPAN_TARGET`) are at INFO, which
//! Bunyan would print as a START and an END line each. `QuietSpans` hides
//! them from the formatter, and the formatter skips their fields, so a line
//! written inside one gains only `trace_id` and `span_id` (FR-004).

use thunderforge_server::telemetry::{SPAN_FIELDS, SPAN_TARGET};
use tracing::span::{Attributes, Id};
use tracing::{Event, Subscriber};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::Registry;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, filter::Directive};

/// The directive that keeps those spans whatever `RUST_LOG` says.
pub fn directive() -> Directive {
    format!("{SPAN_TARGET}=info")
        .parse()
        .expect("a valid directive")
}

/// `RUST_LOG`, or `info`, plus `directive()`.
pub fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"))
        .add_directive(directive())
}

/// The formatter, with the spans' fields skipped.
pub fn formatter<W>(name: &str, writer: W) -> QuietSpans<BunyanFormattingLayer<W>>
where
    W: for<'a> MakeWriter<'a> + 'static,
{
    let layer = BunyanFormattingLayer::new(name.into(), writer)
        .skip_fields(SPAN_FIELDS.iter().copied())
        .expect("no Bunyan core field is skipped");
    QuietSpans(layer)
}

/// What OpenTelemetry adds to the registry, when telemetry installed any.
pub type OtelLayers = Vec<Box<dyn Layer<Registry> + Send + Sync + 'static>>;

/// The process's subscriber: the OTel layers, `filter`, and Bunyan on
/// `writer`. An empty list goes in as `None`: an empty `Vec` layer answers
/// every callsite `Interest::never()`, which the global dispatcher caches,
/// and the whole log goes quiet.
pub fn subscriber<W>(
    otel: Option<OtelLayers>,
    filter: EnvFilter,
    writer: W,
) -> impl Subscriber + Send + Sync
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    Registry::default()
        .with(otel.filter(|layers| !layers.is_empty()))
        .with(filter)
        .with(JsonStorageLayer)
        .with(formatter("thunderforge", writer))
}

pub struct QuietSpans<L>(pub L);

fn ours(meta: &tracing::Metadata<'_>) -> bool {
    meta.target() == SPAN_TARGET
}

impl<S, L> Layer<S> for QuietSpans<L>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    L: Layer<S>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if !ours(attrs.metadata()) {
            self.0.on_new_span(attrs, id, ctx);
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let skip = ctx.span(&id).is_some_and(|s| ours(s.metadata()));
        if !skip {
            self.0.on_close(id, ctx);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        self.0.on_event(event, ctx);
    }
}

/// A writer into a shared buffer, for the tests here and in `http_tests`.
#[cfg(test)]
#[derive(Clone, Default)]
pub struct Captured(pub std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

#[cfg(test)]
impl Captured {
    pub fn lines(&self) -> Vec<serde_json::Value> {
        let bytes = self.0.lock().unwrap().clone();
        String::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).expect("a JSON line"))
            .collect()
    }
}

#[cfg(test)]
impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        std::io::Write::write(&mut *self.0.lock().unwrap(), buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
impl<'a> MakeWriter<'a> for Captured {
    type Writer = Captured;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(line: &serde_json::Value) -> Vec<String> {
        let mut k: Vec<String> = line.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    }

    #[test]
    fn our_spans_print_nothing_and_add_no_fields() {
        let plain = Captured::default();
        let quiet = Captured::default();
        let sub = Registry::default()
            .with(JsonStorageLayer)
            .with(formatter("t", plain.clone()));
        tracing::subscriber::with_default(sub, || {
            tracing::info!(user = 1, "outside");
        });
        let sub = Registry::default()
            .with(JsonStorageLayer)
            .with(formatter("t", quiet.clone()));
        tracing::subscriber::with_default(sub, || {
            let span = tracing::info_span!(
                target: SPAN_TARGET,
                "graphql.operation",
                otel.name = "graphql.mutation doIt",
                graphql.root_field = "doIt",
                outcome = "ok",
                event = "roll_made",
                world.id = "w",
            );
            let _e = span.enter();
            tracing::info!(user = 1, "outside");
        });
        let (plain, quiet) = (plain.lines(), quiet.lines());
        assert_eq!(quiet.len(), 1, "no START or END: {quiet:?}");
        assert_eq!(keys(&plain[0]), keys(&quiet[0]));
    }

    #[test]
    fn other_spans_still_print_start_and_end() {
        let out = Captured::default();
        let sub = Registry::default()
            .with(JsonStorageLayer)
            .with(formatter("t", out.clone()));
        tracing::subscriber::with_default(sub, || {
            let span = tracing::info_span!("job");
            let _e = span.enter();
        });
        let lines = out.lines();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["msg"], "[JOB - START]");
        assert_eq!(lines[1]["msg"], "[JOB - END]");
    }

    /// The OTel layers are what telemetry installed: none when it is off,
    /// and possibly an empty list. An empty `Vec` layer answers every
    /// callsite with `Interest::never()`, which would silence the whole
    /// log, the setup link with it.
    #[test]
    fn every_line_prints_whatever_telemetry_installed() {
        for otel in [None, Some(Vec::new())] {
            let out = Captured::default();
            let sub = subscriber(otel, EnvFilter::new("info"), out.clone());
            // A scoped dispatcher falls back to `enabled()`, which an empty
            // `Vec` passes, so ask what the global one would cache.
            let meta = tracing::warn_span!("probe")
                .metadata()
                .expect("span metadata");
            assert!(
                !sub.register_callsite(meta).is_never(),
                "the callsite is silenced"
            );
            tracing::subscriber::with_default(sub, || {
                tracing::warn!("Initial admin setup: /setup/abc12345");
            });
            let lines = out.lines();
            assert_eq!(lines.len(), 1, "{lines:?}");
            assert_eq!(lines[0]["msg"], "Initial admin setup: /setup/abc12345");
        }
    }

    #[test]
    fn the_directive_passes_our_target_under_a_quiet_rust_log() {
        let filter = EnvFilter::new("warn").add_directive(directive());
        assert!(filter.to_string().contains("thunderforge::otel=info"));
    }
}
