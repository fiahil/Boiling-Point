//! The export-boundary redaction control (Constitution I, extended).
//!
//! [`RedactingExporter`] wraps any OTLP [`SpanExporter`] and strips every span
//! attribute whose key is not on the [`span_schema`] public allow-list **before**
//! delegating to the inner exporter. Redaction is allow-list / fail-closed: a key
//! that is neither public nor a known secret is dropped, so a newly added secret
//! cannot leak before the schema catches up. Secrets therefore ride spans
//! in-process (for the privileged reveal and the projection) but never leave the
//! process via telemetry.

use std::future::Future;

use opentelemetry::KeyValue;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::{SpanData, SpanExporter};

use super::span_schema;

/// Strip every attribute whose key is not on the public allow-list. This is the
/// single redaction primitive; both the exporter and its security test call it, so
/// the test exercises exactly what the boundary does.
pub(crate) fn redact_attributes(attributes: &mut Vec<KeyValue>) {
    attributes.retain(|kv| span_schema::is_public(kv.key.as_str()));
}

/// A [`SpanExporter`] that redacts secret attributes before handing spans to the
/// wrapped exporter.
#[derive(Debug)]
pub struct RedactingExporter<E> {
    inner: E,
}

impl<E> RedactingExporter<E> {
    /// Wrap `inner` so every exported span is redacted first.
    pub fn new(inner: E) -> Self {
        Self { inner }
    }
}

impl<E: SpanExporter> SpanExporter for RedactingExporter<E> {
    fn export(&self, mut batch: Vec<SpanData>) -> impl Future<Output = OTelSdkResult> + Send {
        for span in &mut batch {
            redact_attributes(&mut span.attributes);
        }
        self.inner.export(batch)
    }

    fn shutdown(&mut self) -> OTelSdkResult {
        self.inner.shutdown()
    }

    fn force_flush(&mut self) -> OTelSdkResult {
        self.inner.force_flush()
    }

    fn set_resource(&mut self, resource: &Resource) {
        self.inner.set_resource(resource);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::span_schema::{SECRET_ATTRS, attr};

    /// The security control: a span populated with every enumerated secret
    /// attribute, plus public ones, must come out carrying no secret key. The
    /// secret keys are derived from the authoritative set, so adding a new secret
    /// without allow-listing it stays redacted by construction.
    #[test]
    fn no_secret_attribute_survives_redaction() {
        let mut attrs: Vec<KeyValue> = SECRET_ATTRS
            .iter()
            .map(|k| KeyValue::new(*k, "SECRET-VALUE"))
            .collect();
        // Public attributes that must survive.
        attrs.push(KeyValue::new(attr::ROOM_CODE, "ABCD"));
        attrs.push(KeyValue::new(attr::ROUND_NUMBER, 3_i64));
        // An unknown (neither public nor secret) attribute: must be dropped.
        attrs.push(KeyValue::new("code.filepath", "src/session.rs"));

        redact_attributes(&mut attrs);

        let surviving: Vec<&str> = attrs.iter().map(|kv| kv.key.as_str()).collect();
        for secret in SECRET_ATTRS {
            assert!(
                !surviving.contains(secret),
                "secret attribute {secret} survived redaction"
            );
        }
        assert!(
            !surviving.contains(&"code.filepath"),
            "unknown attribute leaked (fail-open) — redaction must be fail-closed"
        );
        assert!(
            surviving.contains(&attr::ROOM_CODE),
            "public room.code was stripped"
        );
        assert!(
            surviving.contains(&attr::ROUND_NUMBER),
            "public round.number was stripped"
        );
    }

    /// Redaction over a span with only secrets yields an empty attribute set.
    #[test]
    fn all_secret_span_is_fully_stripped() {
        let mut attrs = vec![
            KeyValue::new(attr::BOILING_POINT, 11_i64),
            KeyValue::new(attr::HAND, "R3,B1,Y2"),
            KeyValue::new(attr::DECK_SEED, 42_i64),
        ];
        redact_attributes(&mut attrs);
        assert!(attrs.is_empty(), "no public keys, so nothing should export");
    }
}
