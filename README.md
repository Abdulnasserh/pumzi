# PUMZI

A small desktop companion that taps you on the shoulder every 20 minutes or so to rest your eyes, sip water, breathe, and move. It is a Tauri rewrite of [Noodle](https://github.com/pcybriwsky/noodle) with extra break categories and a dashboard.

No Node, npm, or Electron. The backend is Rust; the UI is plain HTML/CSS/JS rendered by the system webview.

## What it does

- **23 breaks across four categories** — eyes (4), hydration (3), breathing (4), movement (12).
- **Calm break card** — appears in a corner, never steals focus, and measures itself so it fits the content.
- **Rotation without repeats** — shuffles categories first, then breaks within a category, so you get a balanced mix.
- **Pomodoro focus tasks** — give a task a focus budget; PUMZI splits it into 25-minute sessions with 5-minute breaks, a 15-minute long break every four sessions, and an alarm + notification when a break is due.
- **Dashboard** — focus countdown, task list, pause/resume, today's count, streak, seven-day chart, recent breaks.
- **Reminders that respect your time** — quiet hours (including across midnight), snooze, away detection, and idle hold.
- **Tray menu** — task countdown only while focus is running, take a break, pause/resume, dashboard, quit.
- **Local only** — config and stats live in JSON files on your machine. No account, network, or telemetry.

## Requirements

- Rust 1.77 or newer
- macOS: Xcode Command Line Tools (`xcode-select --install`)
- Windows: WebView2 (installed by default on Windows 10/11)
- Linux: `webkit2gtk-4.1`, `libappindicator3`, and the usual Tauri build dependencies

## Run it

```sh
cd desktop/src-tauri
cargo run
```

The first launch opens the dashboard. After that, PUMZI lives in the tray/menu bar. Use `cargo run -- --demo` to open the dashboard and a break card immediately.

Run the tests with:

```sh
cargo test
```

## Package it

The build works without npm. Install the Tauri CLI once:

```sh
cargo install tauri-cli --locked
```

Then build the app bundle from `desktop/`:

```sh
cargo tauri build
```

Bundles land in `desktop/src-tauri/target/release/bundle/`.

## Using it

- Add a task in the dashboard, then press **Start**. The countdown and tray timer only appear after you start a task.
- A task is a focus-time budget (for example 50 minutes). PUMZI breaks it into Pomodoro sessions and schedules breaks between them. **End task** stops the timer.
- Click the tray icon for controls.
- In a break card: **Start** begins the timer, **Done** records the break, **Snooze** delays it, and **Skip** moves on.
- The card closes itself after a while if you ignore it, so it never piles up.
- The dashboard has reminders, snooze length, card position, quiet hours, break mix, sound, login item, and reset controls.

Settings are written to the platform app-data directory, for example:

- macOS: `~/Library/Application Support/com.pumzi.app/config.json`
- Windows: `%APPDATA%\com.pumzi.app\config.json`
- Linux: `~/.config/com.pumzi.app/config.json`

Stats are stored next to the config in `stats.json`. Tasks and the active Pomodoro live in `tasks.json`.

## Project layout

```
desktop/
├── assets/figures/        animated SVGs for every break
├── data/breaks.json       23 break definitions and category copy
├── renderer/              break card + dashboard (plain web UI)
├── scripts/make-icons.py  regenerates app and tray icons
└── src-tauri/             Rust backend, tray, windows, scheduler
    ├── src/config.rs      defaults, validation, partial updates
    ├── src/scheduler.rs   intervals, quiet hours, idle/away behavior
    ├── src/tasks.rs       Pomodoro sessions, breaks, pause/resume
    ├── src/rotation.rs    category and break shuffle bags
    ├── src/stats.rs       daily totals, streak, seven-day summary
    ├── src/store.rs       atomic JSON persistence
    └── src/main.rs        windows, tray, commands, ticker
```

## Notes

- Developed and smoke-tested on macOS 12.7.6. The Tauri code is cross-platform, but Windows and Linux have not been tested yet.
- Idle detection currently uses CoreGraphics on macOS. On Windows and Linux the app still runs; “reset when away” simply has no idle signal yet.
- The break card uses a transparent, always-on-top, non-activating window on macOS.
- Icons can be regenerated with `python3 scripts/make-icons.py` (uses `sips` and `iconutil` on macOS).
- Break content is adapted from the original Noodle project. MIT licensed.
