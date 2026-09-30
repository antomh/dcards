//! Reading the currently selected text.
//!
//! X11 PRIMARY (the mouse selection) takes priority over CLIPBOARD, so that a
//! word that is merely selected is used even when something else was copied
//! earlier. PRIMARY is read with `x11-clipboard`, trying `UTF8_STRING` first
//! and then `STRING` (some application serve only the latter); if that fails,
//! `arboard` is asked for the primary selection too (which also covers
//! Wayland). CLIPBOARD is the last resort.

use std::time::Duration;

use arboard::GetExtLinux;

/// How long to wait for the PRIMARY selection owner to answer.
const PRIMARY_TIMEOUT: Duration = Duration::from_millis(500);

/// Read the current selection as trimmed text.
pub fn read_selection() -> Option<String> {
    choose(read_primary(), read_clipboard())
}

/// Pick the selection, preferring PRIMARY, and trim it.
///
/// Extracted so the priority rule can be tested without an X server.
fn choose(primary: Option<String>, clipboard: Option<String>) -> Option<String> {
    primary
        .filter(|text| !text.trim().is_empty())
        .or_else(|| clipboard.filter(|text| !text.trim().is_empty()))
        .map(|text| text.trim().to_string())
}

/// The PRIMARY selection, via X11 first and `arboard` as a fallback.
fn read_primary() -> Option<String> {
    x11_primary().or_else(arboard_primary)
}

fn x11_primary() -> Option<String> {
    let clipboard = x11_clipboard::Clipboard::new().ok()?;
    let atoms = &clipboard.getter.atoms;

    let utf8 = clipboard
        .load(
            atoms.primary,
            atoms.utf8_string,
            atoms.property,
            PRIMARY_TIMEOUT,
        )
        .ok()
        .filter(|bytes| !bytes.is_empty());

    let bytes = match utf8 {
        Some(bytes) => bytes,
        None => clipboard
            .load(atoms.primary, atoms.string, atoms.property, PRIMARY_TIMEOUT)
            .ok()
            .filter(|bytes| !bytes.is_empty())?,
    };

    Some(decode(bytes))
}

fn arboard_primary() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    clipboard
        .get()
        .clipboard(arboard::LinuxClipboardKind::Primary)
        .text()
        .ok()
}

fn read_clipboard() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    clipboard.get_text().ok()
}

/// Decode selection bytes: UTF-8 when possible, ISO-8859-1 otherwise.
fn decode(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(err) => err.into_bytes().iter().map(|&byte| byte as char).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_wins_over_clipboard() {
        assert_eq!(
            choose(Some(" selected ".into()), Some("copied".into())),
            Some("selected".to_string())
        );
    }

    #[test]
    fn empty_primary_falls_back_to_clipboard() {
        assert_eq!(
            choose(Some("   ".into()), Some("copied".into())),
            Some("copied".to_string())
        );
        assert_eq!(
            choose(None, Some(" copied ".into())),
            Some("copied".to_string())
        );
    }

    #[test]
    fn empty_everything_is_none() {
        assert_eq!(choose(None, None), None);
        assert_eq!(choose(Some(String::new()), Some("  ".into())), None);
    }

    #[test]
    fn decode_utf8_and_latin1() {
        assert_eq!(decode("сердце".as_bytes().to_vec()), "сердце");
        // 0xE9 is 'é' in ISO-8859-1.
        assert_eq!(decode(vec![0xE9]), "é");
    }
}
