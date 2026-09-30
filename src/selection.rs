//! Reading the currently selected text.
//!
//! X11 PRIMARY is preferred; if it is empty or unavailable (for instance on
//! Wayland) the CLIPBOARD is used instead. The result is trimmed, and empty
//! text is reported as `None`.

use std::time::Duration;

/// How long to wait for the PRIMARY selection owner to answer.
const PRIMARY_TIMEOUT: Duration = Duration::from_millis(200);

/// Read the current selection as trimmed text.
pub fn read_selection() -> Option<String> {
    read_primary()
        .filter(|text| !text.trim().is_empty())
        .or_else(read_clipboard)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

/// Read the X11 PRIMARY selection via `x11-clipboard`.
fn read_primary() -> Option<String> {
    let clipboard = x11_clipboard::Clipboard::new().ok()?;
    let bytes = clipboard
        .load(
            clipboard.getter.atoms.primary,
            clipboard.getter.atoms.utf8_string,
            clipboard.getter.atoms.property,
            PRIMARY_TIMEOUT,
        )
        .ok()?;
    String::from_utf8(bytes).ok()
}

/// Read the CLIPBOARD via `arboard`.
fn read_clipboard() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    clipboard.get_text().ok()
}
