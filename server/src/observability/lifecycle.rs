//! The in-process span-lifecycle hook: a `tracing` `Layer` that observes span
//! **start** and **end** for the documented span tree and forwards them to a
//! registered consumer (the `admin-ui` projection lives in a separate change).
//!
//! Two properties matter and are structural here, not conventions:
//! - **Upstream of export sampling.** This is a plain `tracing` `Layer`, so it sees
//!   100% of spans regardless of any sampling on the OTEL export path — exact
//!   balance aggregates (Constitution IV) need the unsampled stream.
//! - **Never backpressures the game loop.** Events cross to the consumer over a
//!   bounded channel via `try_send`; when the consumer is slow and the buffer
//!   fills, events are **dropped** (counted) rather than blocking the emitter.
//!
//! The seam hands consumers only owned, observed [`SpanEvent`]s — no handle to game
//! state — so it is read-only by construction.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, OnceLock};

use arc_swap::ArcSwapOption;
use tracing::Subscriber;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

/// Capacity of the bounded hand-off channel to the consumer. Past this, a slow
/// consumer causes events to be dropped rather than backpressuring emission.
const CHANNEL_CAPACITY: usize = 4096;

/// Whether a [`SpanEvent`] marks a span opening or closing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanEventKind {
    /// The span just opened (`on_new_span`).
    Start,
    /// The span just closed (`on_close`).
    End,
}

/// One observed span-lifecycle event handed to a consumer. Carries owned data
/// only; there is no path back to game state.
#[derive(Debug, Clone)]
pub struct SpanEvent {
    /// Whether the span opened or closed.
    pub kind: SpanEventKind,
    /// The `tracing` span id (stable for the span's lifetime).
    pub id: u64,
    /// The span name (e.g. `room.lifetime`, `round`).
    pub name: &'static str,
    /// The parent span's id, if any — used to reconstruct the live tree.
    pub parent_id: Option<u64>,
    /// The span's attributes (public and secret) as captured in-process.
    pub attributes: BTreeMap<String, String>,
}

/// A consumer of the span-lifecycle stream. Implementors MUST NOT block in
/// [`on_event`](SpanConsumer::on_event) for long; the hand-off is already
/// decoupled by a bounded channel, but a wedged consumer only starves itself.
pub trait SpanConsumer: Send + Sync + 'static {
    /// Handle one lifecycle event.
    fn on_event(&self, event: SpanEvent);
}

/// A registration slot + drop counter shared between the [`LifecycleLayer`] and the
/// registration API. Cloning shares the same underlying slot.
#[derive(Clone, Default)]
pub struct LifecycleHandle {
    sender: Arc<ArcSwapOption<SyncSender<SpanEvent>>>,
    dropped: Arc<AtomicU64>,
}

impl LifecycleHandle {
    /// A fresh, unregistered handle.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `consumer` to receive lifecycle events. Spawns a drain thread that
    /// forwards events from the bounded channel to the consumer, so a slow consumer
    /// never blocks the emitting (game-loop) threads. Replacing a prior consumer
    /// drops the old channel (its drain thread then exits).
    pub fn register(&self, consumer: Arc<dyn SpanConsumer>) {
        let (tx, rx) = sync_channel::<SpanEvent>(CHANNEL_CAPACITY);
        std::thread::Builder::new()
            .name("span-lifecycle-drain".into())
            .spawn(move || {
                while let Ok(event) = rx.recv() {
                    consumer.on_event(event);
                }
            })
            .expect("spawn span-lifecycle drain thread");
        self.sender.store(Some(Arc::new(tx)));
    }

    /// Number of events dropped because the consumer's buffer was full.
    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// A `tracing` layer feeding this handle.
    pub fn layer(&self) -> LifecycleLayer {
        LifecycleLayer {
            handle: self.clone(),
        }
    }

    /// Forward one event, dropping it (and counting) if no consumer is registered
    /// or its buffer is full. Never blocks.
    fn emit(&self, event: SpanEvent) {
        let Some(tx) = self.sender.load_full() else {
            return; // no consumer registered — nothing to do
        };
        match tx.try_send(event) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// The process-wide handle used by the installed subscriber.
static GLOBAL: OnceLock<LifecycleHandle> = OnceLock::new();

/// The global lifecycle handle (created on first use).
pub fn global_handle() -> &'static LifecycleHandle {
    GLOBAL.get_or_init(LifecycleHandle::new)
}

/// Register the process-wide span-lifecycle consumer (e.g. the admin projection).
pub fn register_consumer(consumer: Arc<dyn SpanConsumer>) {
    global_handle().register(consumer);
}

/// Per-span data retained between start and close so the End event carries the same
/// name/parent/attributes as the Start event.
struct StoredSpan {
    name: &'static str,
    parent_id: Option<u64>,
    attributes: BTreeMap<String, String>,
}

/// Collects `tracing` span fields into a string map.
struct FieldVisitor<'a>(&'a mut BTreeMap<String, String>);

impl Visit for FieldVisitor<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

/// The span-lifecycle `Layer`. Observes start/end and forwards [`SpanEvent`]s to its
/// handle's consumer.
pub struct LifecycleLayer {
    handle: LifecycleHandle,
}

impl<S> Layer<S> for LifecycleLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut attributes = BTreeMap::new();
        attrs.record(&mut FieldVisitor(&mut attributes));
        let name = attrs.metadata().name();
        let parent_id = attrs
            .parent()
            .cloned()
            .or_else(|| ctx.lookup_current().map(|s| s.id()))
            .map(|id| id.into_u64());

        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(StoredSpan {
                name,
                parent_id,
                attributes: attributes.clone(),
            });
        }

        self.handle.emit(SpanEvent {
            kind: SpanEventKind::Start,
            id: id.into_u64(),
            name,
            parent_id,
            attributes,
        });
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id)
            && let Some(stored) = span.extensions_mut().get_mut::<StoredSpan>()
        {
            values.record(&mut FieldVisitor(&mut stored.attributes));
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(&id)
            && let Some(stored) = span.extensions().get::<StoredSpan>()
        {
            self.handle.emit(SpanEvent {
                kind: SpanEventKind::End,
                id: id.into_u64(),
                name: stored.name,
                parent_id: stored.parent_id,
                attributes: stored.attributes.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use tracing_subscriber::prelude::*;

    /// A consumer that records every event it sees.
    #[derive(Default)]
    struct RecordingConsumer(Mutex<Vec<SpanEvent>>);
    impl RecordingConsumer {
        fn events(&self) -> Vec<SpanEvent> {
            self.0.lock().unwrap().clone()
        }
    }
    impl SpanConsumer for RecordingConsumer {
        fn on_event(&self, event: SpanEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    /// A deliberately slow consumer, to prove emission is not backpressured.
    struct SlowConsumer;
    impl SpanConsumer for SlowConsumer {
        fn on_event(&self, _event: SpanEvent) {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Spin until `f` is true or the deadline passes (the drain thread is async).
    fn wait_until(mut f: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if f() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        f()
    }

    #[test]
    fn consumer_sees_span_start_and_end_with_attributes() {
        let handle = LifecycleHandle::new();
        let consumer = Arc::new(RecordingConsumer::default());
        handle.register(consumer.clone());

        let subscriber = tracing_subscriber::registry().with(handle.layer());
        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("room.lifetime", room.code = "ABCD");
            span.in_scope(|| {});
            drop(span);
        });

        assert!(
            wait_until(|| {
                let evs = consumer.events();
                evs.iter().any(|e| e.kind == SpanEventKind::Start)
                    && evs.iter().any(|e| e.kind == SpanEventKind::End)
            }),
            "expected both a Start and an End event"
        );

        let events = consumer.events();
        let start = events
            .iter()
            .find(|e| e.kind == SpanEventKind::Start)
            .unwrap();
        assert_eq!(start.name, "room.lifetime");
        assert_eq!(
            start.attributes.get("room.code").map(String::as_str),
            Some("ABCD")
        );
        assert!(
            events
                .iter()
                .any(|e| e.kind == SpanEventKind::End && e.id == start.id)
        );
    }

    #[test]
    fn slow_consumer_does_not_backpressure_emission() {
        let handle = LifecycleHandle::new();
        handle.register(Arc::new(SlowConsumer));

        let subscriber = tracing_subscriber::registry().with(handle.layer());
        let elapsed = tracing::subscriber::with_default(subscriber, || {
            let start = Instant::now();
            // Far more spans than the channel can hold while the consumer crawls.
            for _ in 0..20_000 {
                let span = tracing::info_span!("wave", wave.number = 1_u64);
                span.in_scope(|| {});
            }
            start.elapsed()
        });

        // Emission must stay fast despite the 20ms-per-event consumer: 20k events
        // through a slow consumer would take minutes if it backpressured.
        assert!(
            elapsed < Duration::from_secs(5),
            "emission was backpressured by the slow consumer: took {elapsed:?}"
        );
        assert!(
            handle.dropped_count() > 0,
            "expected events to be dropped when the consumer fell behind"
        );
    }
}
