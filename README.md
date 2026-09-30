# dcards

A flashcard dictionary for Linux (X11/XFCE). Select a word, press a global
hotkey, and dcards asks an OpenAI-compatible LLM for a translation or
definition, then lets you keep it as a flashcard.

> **Status: Stage 0.** This commit contains the process skeleton only:
> configuration, logging, single-instance handling, the tray/hotkey
> integrations and an empty window. Persistence, the LLM client and the card UI
> arrive in later stages.

## Requirements

- Linux with an X11 session (Wayland is out of scope for the MVP).
- A D-Bus session bus (for the tray icon and notifications).
- A StatusNotifier (AppIndicator/SNI) host for the tray icon — see
  [Tray](#tray).
- Build dependencies for the tray backend:
  `libgtk-3-dev` and `libayatana-appindicator3-dev` (or `libappindicator3-dev`).
  On Debian/Ubuntu/Mint:

  ```sh
  sudo apt install libgtk-3-dev libayatana-appindicator3-dev
  ```

- Rust 1.75+ (edition 2021).

## Build and run

```sh
cargo run
```

## Files

| Purpose | Path |
| --- | --- |
| Configuration | `~/.config/dcards/config.toml` |
| Database (later stage) | `~/.local/share/dcards/dcards.db` |
| Logs | `~/.local/state/dcards/logs/dcards.<date>.log` |
| Single-instance socket | `$XDG_RUNTIME_DIR/dcards.sock` |

`config.toml` is created on first run with permissions `0600` (the temporary
file used for atomic saves is created with `0600` as well).

## Configuration

See the generated `config.toml`. Fields of note:

- `general.language_pair`: `en-en`, `en-ru` or `ru-en`.
- `general.default_group`: name of the default group.
- `general.cards_limit`: maximum cards shown per group.
- `general.theme`: `dark` or `light`.
- `[llm]`: `base_url`, `api_key`, `model`, timeout and sampling settings.
- `[prompts]`: per-pair system prompts (`en_en`, `en_ru`, `ru_en`).
- `[logging]`: level, retained file count, slow-query threshold.

### Secrets

The LLM API key is stored in `config.toml` (mode `0600`). It is **never**
written to the log, embedded in error messages, or printed through `Debug`
(`LlmConfig` has a redacting `Debug` implementation).

## Global hotkey

The fixed combination is **Ctrl+Alt+S**. (`Ctrl+Alt+D` is taken by XFCE's
"Show Desktop".) If registration fails, the application keeps running and only a
warning is logged; no notification is shown, because the same action will be
available from the card draft once that stage lands. On a Wayland session the
hotkey is deliberately not registered and a warning is shown in the window.

## Single instance

The first process binds `$XDG_RUNTIME_DIR/dcards.sock`. A second invocation
connects to it, asks the running instance to raise its window, and exits.
Stale sockets from crashed processes are detected and replaced.

## Tray

The tray icon uses the AppIndicator/StatusNotifierItem backend of `tray-icon`.
GTK must run a main loop for this backend, so GTK is initialised and driven on a
dedicated worker thread (the eframe/winit event loop owns the main thread). The
menu contains **Open dcards**, **New card** and **Quit**. If the tray cannot be
created it is skipped with a warning and the rest of the application keeps
working.

### Linux Mint / XFCE: enabling the tray

The XFCE panel on Linux Mint uses the XAppStatusIcon mechanism. SNI/AppIndicator
icons (what dcards uses, and what `nm-applet`, blueman, etc. use) are bridged to
it by the `xapp-sn-watcher` service — but that service deliberately refuses to
start unless the current desktop is listed in the dconf key
`org.x.apps.statusicon status-notifier-enabled-desktops`. On Mint this list
contains `MATE`, `Cinnamon` and `X-Cinnamon`, but not `XFCE`.

Add `XFCE` to the list and restart the panel (or log out and back in):

```sh
gsettings set org.x.apps.statusicon status-notifier-enabled-desktops \
  "['MATE', 'Cinnamon', 'X-Cinnamon', 'XFCE']"
```

After that `xapp-sn-watcher` runs, exposes `org.kde.StatusNotifierWatcher`, and
the panel shows the dcards icon. Without it there is simply no StatusNotifier
host on the session and no SNI icon can be displayed.

## Spike results (Stage 0)

Measured on the development machine: Linux Mint 22.3, XFCE, X11, D-Bus session.
Logs are written to `~/.local/state/dcards/logs/`.

- **(a) Tray icon.** `tray-icon`'s AppIndicator backend requires GTK, so the
  tray is created on a dedicated GTK thread. The icon registers as
  `org.kde.StatusNotifierItem-<pid>-1`. Root cause of the initial failure:
  the session had **no** `org.kde.StatusNotifierWatcher`, because Mint's
  `xapp-sn-watcher` bridge refuses to start for `XDG_CURRENT_DESKTOP=XFCE`
  (see [Tray](#tray)). After adding `XFCE` to
  `org.x.apps.statusicon status-notifier-enabled-desktops`, the watcher runs and
  dcards appears among the registered items — the criterion passes.
- **(b) Hotkey.** `Ctrl+Alt+S` registers successfully
  (`registered global hotkey Ctrl+Alt+S` in the log). If another instance
  already holds it, registration fails with a warning and the app keeps running.
- **(c) PRIMARY selection.** Reading works: a value stored into PRIMARY can be
  fetched back with `x11-clipboard`; CLIPBOARD read/write works with `arboard`.
  The `PRIMARY -> CLIPBOARD` fallback for stage 4 is therefore viable.

Environment note: in the sandbox used for the spike, `XDG_STATE_HOME` was set,
so logs landed under that directory. On a normal session they go to
`~/.local/state/dcards/logs`.

## Limitations (MVP)

- X11 only; the global hotkey and PRIMARY selection do not work on Wayland.
- The tray requires a StatusNotifier host (see [Tray](#tray)).
- Single theme per configuration; no per-word statistics or review ratings.
