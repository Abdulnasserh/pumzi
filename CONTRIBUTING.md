# Contributing to PUMZI

Thanks for wanting to help! 🫁 PUMZI is a small, deliberately simple project — and simple is a feature. This guide tells you what helps and what to avoid.

## Ground rules

- **No npm, no Node, no Electron.** The backend is Rust; the UI is plain HTML/CSS/JS rendered by the system webview. Please don't introduce a package manager or a framework.
- **Local only.** No accounts, no network calls, no telemetry, no analytics. Ever.
- **Calm by design.** PUMZI's job is to respect focus, not compete with it. If a change makes the app louder, pushier, or busier, it probably doesn't belong here.

## Getting started

```sh
git clone https://github.com/Abdulnasserh/pumzi.git
cd pumzi/desktop/src-tauri
cargo run          # first launch opens the dashboard
cargo run -- --demo  # dashboard + a break card immediately
```

Before opening a PR, make sure:

```sh
cargo test       # all tests pass
cargo clippy     # no warnings
```

## Good first contributions

### 1. Add a break

The easiest and most welcome kind of PR.

1. Add the definition to `data/breaks.json` — pick a category (`eyes`, `hydration`, `breathing`, `movement`), write a short title and instruction in the same tone as the existing ones.
2. Add an animated SVG figure in `assets/figures/<category>/` — keep it hand-drawn feeling and consistent with the existing figures (small, flat, few colors).
3. That's it. The scheduler, rotation bags, and card pick it up automatically.

### 2. Fix a bug

Open an issue first (or check existing ones), describe what happens vs. what should happen, and reference the issue in your PR.

### 3. Platform improvements

Windows and Linux are supported in code but untested in practice — idle detection has no signal there yet. If you run one of those platforms, testing reports and fixes are especially valuable.

## Style

- Rust: follow the existing code style; `cargo fmt` and `cargo clippy` clean.
- Web UI: plain HTML/CSS/JS, no build step. Match the existing cream-and-ink design language.
- Commit messages: short, present tense ("Add wrist-stretch break", "Fix switch click on Linux").

## Submitting

1. Fork, branch off `main`, make your change.
2. Open a PR with a short description of *what* and *why*.
3. Keep PRs small — one change per PR.

## Questions?

Open an issue — happy to talk through ideas before you write code.

PUMZI is [MIT licensed](LICENSE); by contributing you agree your work is released under the same license.
