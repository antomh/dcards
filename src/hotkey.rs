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
    let wayland_display = std::env::var("WAYLAND_DISPLAY").is_ok_and(|v| !v.is_empty());
    let session_type =
        std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v.eq_ignore_ascii_case("wayland"));
    wayland_display || session_type
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
