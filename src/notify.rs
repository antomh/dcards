//! Desktop notifications via `notify-rust`.
//!
//! Notification failures are logged at `warn` level and otherwise ignored: a
//! missing notification daemon must never crash or block the application.

use notify_rust::Notification;

/// Show an informational notification.
pub fn info(summary: &str, body: &str) {
    show(summary, body);
}

/// Show an error notification.
pub fn error(summary: &str, body: &str) {
    show(summary, body);
}

fn show(summary: &str, body: &str) {
    let result = Notification::new()
        .appname("dcards")
        .summary(summary)
        .body(body)
        .show();
    if let Err(err) = result {
        tracing::warn!(error = %err, "failed to show notification");
    }
}
