//! Global hotkey registration.
//!
//! The MVP uses a fixed combination, `Ctrl+Alt+S` (`Ctrl+Alt+D` is taken by
//! XFCE's "Show Desktop"). If registration fails the application keeps working
//! and only a warning is logged — no notification is shown, because the same
//! action is available from the card draft's "Clean and translate" button.
//!
//! `global-hotkey` works on X11. On a Wayland session the hotkey is
//! deliberately not registered.

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::events::{AppEvent, EventSender, Repaint};

/// Whether the current session is Wayland.
pub fn is_wayland() -> bool {
    let session_type = std::env::var("XDG_SESSION_TYPE").ok();
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
    detect_wayland(session_type.as_deref(), wayland_display.as_deref())
}

/// Pure helper behind [`is_wayland`], testable without touching the process
/// environment.
fn detect_wayland(session_type: Option<&str>, wayland_display: Option<&str>) -> bool {
    let by_display = wayland_display.is_some_and(|value| !value.is_empty());
    let by_type = session_type.is_some_and(|value| value.eq_ignore_ascii_case("wayland"));
    by_display || by_type
}

/// Keeps the hotkey manager alive for as long as the application runs.
pub struct HotkeyRegistration {
    _manager: GlobalHotKeyManager,
}

/// Register `Ctrl+Alt+S` and forward presses to the UI.
///
/// Returns an error if the manager cannot be created or the combination is
/// already grabbed; the caller logs it and continues.
pub fn register(tx: EventSender, repaint: Repaint) -> anyhow::Result<HotkeyRegistration> {
    let manager = GlobalHotKeyManager::new()?;
    let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyS);
    manager.register(hotkey)?;

    std::thread::Builder::new()
        .name("dcards-hotkey".to_string())
        .spawn(move || {
            let receiver = GlobalHotKeyEvent::receiver();
            while let Ok(event) = receiver.recv() {
                if event.state() == HotKeyState::Pressed {
                    let _ = tx.send(AppEvent::Hotkey);
                    repaint.notify();
                }
            }
        })?;

    Ok(HotkeyRegistration { _manager: manager })
}

#[cfg(test)]
mod tests {
    use super::detect_wayland;

    #[test]
    fn detects_wayland_sessions() {
        assert!(detect_wayland(Some("wayland"), None));
        assert!(detect_wayland(Some("Wayland"), None));
        assert!(detect_wayland(None, Some("wayland-0")));
        assert!(detect_wayland(Some("x11"), Some("wayland-0")));
    }

    #[test]
    fn detects_x11_sessions() {
        assert!(!detect_wayland(Some("x11"), None));
        assert!(!detect_wayland(Some("X11"), Some("")));
        assert!(!detect_wayland(None, None));
        assert!(!detect_wayland(None, Some("")));
    }
}
