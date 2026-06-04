//! Regression: the in-process span-lifecycle hook (and thus the admin projection)
//! must observe the server's spans regardless of `RUST_LOG`. A `warn` log level
//! gags the JSON logs but MUST NOT blind the admin surface — otherwise an operator
//! running `RUST_LOG=warn` in production gets a blank inspector and zero balance
//! figures while the game runs normally.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use boiling_point_server::observability::{
    self,
    lifecycle::{SpanConsumer, SpanEvent},
};

/// Records the names of spans the lifecycle hook delivers.
#[derive(Default)]
struct Capture(Mutex<Vec<String>>);
impl SpanConsumer for Capture {
    fn on_event(&self, ev: SpanEvent) {
        self.0.lock().unwrap().push(ev.name.to_string());
    }
}

#[test]
fn lifecycle_hook_sees_info_spans_even_when_rust_log_is_warn() {
    // Gag the logs to `warn`. The lifecycle hook must still see `info_span!`s.
    // SAFETY: set before `init` installs the subscriber; single-threaded test start.
    unsafe {
        std::env::set_var("RUST_LOG", "warn");
    }
    // `None` log level → fall back to `RUST_LOG` (set to `warn` above).
    observability::init("127.0.0.1:0".parse().expect("metrics addr"), None);

    let cap = Arc::new(Capture::default());
    observability::lifecycle::register_consumer(cap.clone());

    // An `info`-level span (every game span is `info_span!`); dropping closes it.
    {
        let _s = tracing::info_span!("group.lifetime", group.code = "ZZZZ").entered();
    }

    // The drain thread is asynchronous — spin briefly for delivery.
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut seen = false;
    while Instant::now() < deadline {
        if cap.0.lock().unwrap().iter().any(|n| n == "group.lifetime") {
            seen = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        seen,
        "the lifecycle consumer must observe info spans even with RUST_LOG=warn"
    );
}
