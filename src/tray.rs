//! System-tray icon and menu.
//!
//! The tray uses the AppIndicator backend of `tray-icon`, which requires a
//! running GTK main loop. Since the eframe/winit event loop already owns the
//! main thread, GTK is initialised and driven on a dedicated worker thread; the
//! icon is created there and menu/click events are forwarded to the UI through
//! the shared event channel.
//!
//! If GTK or the tray cannot be initialised (no D-Bus session, no
//! StatusNotifier host, ...), initialization fails gracefully: the application
//! keeps running and notifications continue to work.

use std::sync::mpsc;
use std::time::Duration;

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::events::{AppEvent, EventSender, Repaint};

const ICON_PNG: &[u8] = include_bytes!("../assets/dcards.png");

const ID_OPEN: &str = "open";
const ID_NEW_CARD: &str = "new_card";
const ID_QUIT: &str = "quit";

/// How long to wait for the GTK thread to report success or failure.
const READY_TIMEOUT: Duration = Duration::from_secs(5);

/// Keeps the tray thread alive for the lifetime of the application.
pub struct TrayGuard {
    _thread: std::thread::JoinHandle<()>,
}

/// Create the tray icon and menu on a dedicated GTK thread.
pub fn init(tx: EventSender, repaint: Repaint) -> anyhow::Result<TrayGuard> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

    let thread = std::thread::Builder::new()
        .name("dcards-tray".to_string())
        .spawn(move || match gtk::init() {
            Err(err) => {
                let _ = ready_tx.send(Err(format!("gtk init failed: {err}")));
            }
            Ok(()) => match build_tray() {
                Ok(_tray) => {
                    // `_tray` must stay alive while the main loop runs.
                    let _ = ready_tx.send(Ok(()));
                    gtk::main();
                }
                Err(err) => {
                    let _ = ready_tx.send(Err(err.to_string()));
                }
            },
        })?;

    match ready_rx.recv_timeout(READY_TIMEOUT) {
        Ok(Ok(())) => {}
        Ok(Err(message)) => anyhow::bail!("{message}"),
        Err(_) => anyhow::bail!("timed out while creating the tray icon"),
    }

    spawn_menu_forwarder(tx.clone(), repaint.clone());
    spawn_click_forwarder(tx, repaint);

    Ok(TrayGuard { _thread: thread })
}

/// Build the tray icon and menu. Must run on the GTK thread.
fn build_tray() -> anyhow::Result<TrayIcon> {
    let menu = Menu::new();
    let open = MenuItem::with_id(MenuId::new(ID_OPEN), "Open dcards", true, None);
    let new_card = MenuItem::with_id(MenuId::new(ID_NEW_CARD), "New card", true, None);
    let quit = MenuItem::with_id(MenuId::new(ID_QUIT), "Quit", true, None);
    menu.append_items(&[&open, &new_card, &PredefinedMenuItem::separator(), &quit])?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(load_icon()?)
        .with_tooltip("dcards")
        .build()?;

    Ok(tray)
}

/// Decode the embedded PNG into an RGBA [`Icon`].
fn load_icon() -> anyhow::Result<Icon> {
    let image = image::load_from_memory(ICON_PNG)?.into_rgba8();
    let (width, height) = image.dimensions();
    Ok(Icon::from_rgba(image.into_raw(), width, height)?)
}

fn spawn_menu_forwarder(tx: EventSender, repaint: Repaint) {
    std::thread::Builder::new()
        .name("dcards-tray-menu".to_string())
        .spawn(move || {
            let receiver = MenuEvent::receiver();
            while let Ok(event) = receiver.recv() {
                let app_event = match event.id().as_ref() {
                    ID_OPEN => AppEvent::TrayOpen,
                    ID_NEW_CARD => AppEvent::TrayNewCard,
                    ID_QUIT => AppEvent::TrayQuit,
                    _ => continue,
                };
                let _ = tx.send(app_event);
                repaint.notify();
            }
        })
        .ok();
}

fn spawn_click_forwarder(tx: EventSender, repaint: Repaint) {
    std::thread::Builder::new()
        .name("dcards-tray-click".to_string())
        .spawn(move || {
            let receiver = TrayIconEvent::receiver();
            while let Ok(event) = receiver.recv() {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    let _ = tx.send(AppEvent::TrayOpen);
                    repaint.notify();
                }
            }
        })
        .ok();
}
