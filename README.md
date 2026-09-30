# dcards

A flashcard dictionary for Linux (X11/XFCE). Select a word, press a global
hotkey, and dcards asks an OpenAI-compatible LLM for a translation or a simple
definition, then lets you keep it as a flashcard. Cards live in local groups
("dictionaries") and can be practised in a simple review mode or exported to
Anki.

## Features

- Global hotkey that turns the current selection into a draft card.
- Automatic translation/definition via any OpenAI-compatible endpoint.
- Groups of cards, manual editing, date filters, simple review mode.
- Anki-compatible TSV import/export (all-or-nothing import).
- Local SQLite storage; no telemetry.

## Requirements

- Linux with an **X11** session. Wayland is not supported (see
  [Wayland](#wayland)).
- A D-Bus session bus for the tray icon and desktop notifications.
- A StatusNotifier (AppIndicator) host for the tray icon — see [Tray](#tray).
- Build dependencies for the tray backend:

  ```sh
  sudo apt install libgtk-3-dev libayatana-appindicator3-dev
  ```

- Rust 1.75+ (edition 2021).

## Build and run

```sh
cargo run
```

Release build:

```sh
cargo build --release
./target/release/dcards
```

## Files

| Purpose | Path |
| --- | --- |
| Configuration | `~/.config/dcards/config.toml` |
| Database | `~/.local/share/dcards/dcards.db` |
| Logs | `~/.local/state/dcards/logs/dcards.<date>.log` |
| Single-instance socket | `$XDG_RUNTIME_DIR/dcards.sock` |

The configuration file is created on first run with permissions `0600`; the
temporary file used for atomic saves uses the same mode.

## Configuration

`config.toml` is created automatically. Key fields:

```toml
[general]
language_pair = "en-en"       # en-en | en-ru | ru-en
default_group = "General"
cards_limit   = 500
theme         = "dark"        # dark | light

[storage]
# db_path = "/home/user/.local/share/dcards/dcards.db"

[llm]
base_url     = "http://localhost:11434/v1"  # "/chat/completions" is appended
api_key      = ""
model        = "llama3.1"
timeout_secs = 15
max_tokens   = 80
temperature  = 0.2

[prompts]                     # optional overrides; sensible defaults built in
en_en = "..."
en_ru = "..."
ru_en = "..."

[logging]
level              = "info"
max_files          = 7
slow_query_warn_ms = 50
```

- `base_url` is normalised: a trailing slash is removed and `/chat/completions`
  is appended unless it is already present.
- `api_key` is required unless `base_url` points at localhost. Pasting the key
  with surrounding whitespace, quotes or a leading `Bearer ` is fine — it is
  normalised before use.
- Prompts may use the `{source_lang}`, `{target_lang}` and `{word}`
  placeholders. The built-in `en-en` prompt asks for a simple, one-sentence
  definition of at most 12 words; the translation prompts ask for the most
  common everyday word. A model that cannot answer is instructed to reply
  exactly `Translation unavailable`.

### Secrets

The API key is stored only in `config.toml` (mode `0600`). It is never written
to the log, embedded in an error message, or printed through `Debug`: the
`LlmConfig`, `LlmRequest` and `SettingsForm` types all redact it. The log may
contain the request URL, HTTP status and model, but never the key.

## Usage

- **Groups / cards.** The main view lists groups on the left and the selected
  group's cards on the right. Cards can be created, edited and deleted; a
  create/edit dialog and a separate draft window are used for new cards.
- **Hotkey draft.** Select a word and press the hotkey: dcards reads the
  selection, validates it, queries the LLM and opens a draft window (front =
  selection, back = answer). Pressing the hotkey again while the draft is open
  refreshes it. The draft is always on top and has **Save**, **Cancel** and
  **Clean and translate** buttons.
- **Review.** Pick a group and start a session; cards are shuffled and shown one
  at a time (front → "Show back" → "Next"). There are no ratings or statistics
  and nothing is written to the database.
- **Settings.** Language pair, default group, card limit, theme and the LLM
  fields; saved to `config.toml`. **Test connection** sends a real request using
  the values currently in the form and shows the answer or the exact error.

## Global hotkey

The combination is fixed at **Ctrl+Alt+S** (`Ctrl+Alt+D` is taken by XFCE's
"Show Desktop"). If registration fails (for example because another application
already owns it), the application keeps running and **only logs a warning** — no
notification is shown, because the same action is available from the draft's
"Clean and translate" button and the tray menu.

## Single instance

The first process binds `$XDG_RUNTIME_DIR/dcards.sock`. Starting dcards again
connects to the running instance, asks it to raise and focus the main window,
and exits. Stale sockets left by a crashed process are detected and replaced.

## Tray

The tray icon uses the AppIndicator/StatusNotifierItem backend of `tray-icon`;
GTK is driven on a dedicated thread. The menu contains **Open dcards**, **New
card** and **Quit**. If the tray cannot be created, it is skipped with a warning
and the rest of the application keeps working.

A StatusNotifier host is required. On a Linux Mint / XFCE session the
`xapp-sn-watcher` bridge deliberately does not start unless the desktop is
listed in the `org.x.apps.statusicon` `status-notifier-enabled-desktops` dconf
key. To enable the tray there:

```sh
gsettings set org.x.apps.statusicon status-notifier-enabled-desktops \
  "['MATE', 'Cinnamon', 'X-Cinnamon', 'XFCE']"
```

then restart the panel (or log out and back in).

## Anki-TSV import and export

Export writes a UTF-8 tab-separated file:

```
#separator:Tab
#html:false
<front>\t<back>\t<group>
```

Tabs, newlines and carriage returns inside a field are replaced with spaces so
every card stays on one line. **Export** offers the current group or all cards.

Import expects the same three columns. Leading `#` directive lines and blank
lines are skipped; an empty group column uses the configured default group; a
group that does not exist yet is created. The whole file is imported in a single
transaction: any malformed line or database error rolls the entire import back,
the database is left unchanged, and the problem is logged and shown as a
notification.

## Wayland

Wayland is not supported. dcards still starts, but shows a warning in the
window: the global hotkey is not registered and the X11 PRIMARY selection is not
available (the clipboard may still work). Everything else functions normally.
`$XDG_SESSION_TYPE` or `$WAYLAND_DISPLAY` is used to detect the session.

## Logging

Structured logs are written to `~/.local/state/dcards/logs/` with daily rotation
and a bounded number of files. The level and retention are set under
`[logging]`. The API key is never logged.

## Limitations (MVP)

- X11 only; no Wayland hotkey or PRIMARY selection.
- The tray needs a StatusNotifier host.
- A single fixed hotkey; no rebinding.
- No per-word statistics, ratings or spaced-repetition scheduling.
- One theme at a time.

## Packaging

`cargo run` (or `cargo build --release`) is the supported way to run the MVP. An
**AppImage** is planned as post-MVP work.
