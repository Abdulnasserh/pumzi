#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod breaks;
mod config;
mod rotation;
mod scheduler;
mod stats;
mod store;
mod tasks;

use std::collections::BTreeMap;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration as StdDuration, Instant};

use auto_launch::AutoLaunchBuilder;
use chrono::Local;
use serde_json::{json, Value};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use breaks::{BreakItem, Breaks};
use rotation::Rotation;
use scheduler::{format_duration, Scheduler, Status};
use stats::Summary;
use store::Store;
use tasks::FocusEvent;

const CARD_WIDTH: f64 = 372.0;
const CARD_INSET: f64 = 16.0;

static QUITTING: AtomicBool = AtomicBool::new(false);

struct AppState {
    store: Store,
    scheduler: Scheduler,
    rotation: Rotation,
    breaks: Breaks,
    card_open: bool,
    current: Option<BreakItem>,
    pending_scheduled: bool,
    safety_deadline: Option<Instant>,
    card_height: f64,
    last_menu_key: String,
    last_tray_title: String,
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;

            let mut store = Store::load(data_dir);
            let first_run = store.config.first_run;
            if first_run {
                store.update_config(&json!({ "firstRun": false }));
            }
            let breaks = breaks::load();
            let scheduler = Scheduler::new(&store.config, now_ms());
            let rotation = Rotation::new(breaks.breaks.clone());

            app.manage(Mutex::new(AppState {
                store,
                scheduler,
                rotation,
                breaks,
                card_open: false,
                current: None,
                pending_scheduled: false,
                safety_deadline: None,
                card_height: 280.0,
                last_menu_key: String::new(),
                last_tray_title: String::new(),
            }));

            create_windows(app)?;

            if let Err(error) = create_tray(app.handle()) {
                eprintln!("Could not create the tray icon: {error}");
            }

            start_ticker(app.handle().clone());

            if first_run {
                open_dashboard_window(app.handle());
            }

            if std::env::args().any(|arg| arg == "--demo") {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(StdDuration::from_millis(900));
                    open_dashboard_window(&handle);
                    show_break(&handle);
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "dashboard" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if !QUITTING.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                    #[cfg(target_os = "macos")]
                    let _ = window
                        .app_handle()
                        .set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_state,
            update_config,
            card_action,
            resize_card,
            take_break,
            toggle_pause,
            open_dashboard,
            close_dashboard,
            reset_stats,
            add_task,
            start_task,
            stop_task,
            delete_task,
            quit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running PUMZI");
}

fn create_windows(app: &mut tauri::App) -> tauri::Result<()> {
    let card = WebviewWindowBuilder::new(app, "card", WebviewUrl::App("break.html".into()))
        .title("PUMZI")
        .inner_size(CARD_WIDTH, 280.0)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .visible(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(false)
        .focused(false)
        .build()?;
    card.set_always_on_top(true)?;
    #[cfg(target_os = "macos")]
    let _ = card.set_visible_on_all_workspaces(true);

    let dashboard =
        WebviewWindowBuilder::new(app, "dashboard", WebviewUrl::App("dashboard.html".into()))
            .title("PUMZI")
            .inner_size(960.0, 800.0)
            .min_inner_size(700.0, 560.0)
            .resizable(true)
            .visible(false)
            .center()
            .build()?;
    let _ = dashboard.hide();

    Ok(())
}

// ---- Tray ----

fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app, false, "Next break soon", 20)?;
    let icon_bytes: &[u8] = if cfg!(target_os = "macos") {
        include_bytes!("../icons/trayTemplate.png")
    } else {
        include_bytes!("../icons/tray.png")
    };
    let mut builder = TrayIconBuilder::with_id("main")
        .icon(tauri::image::Image::from_bytes(icon_bytes)?)
        .tooltip("PUMZI")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| handle_menu(app, event.id.as_ref()));
    #[cfg(target_os = "macos")]
    {
        builder = builder.icon_as_template(true);
    }
    builder.build(app)?;
    Ok(())
}

fn build_menu(
    app: &AppHandle,
    paused: bool,
    status: &str,
    interval: u32,
) -> tauri::Result<Menu<tauri::Wry>> {
    let heading = MenuItem::with_id(app, "status", status, false, None::<&str>)?;
    let take = MenuItem::with_id(app, "take", "Take a break now", true, None::<&str>)?;
    let pause = MenuItem::with_id(
        app,
        "pause",
        if paused {
            "Resume reminders"
        } else {
            "Pause reminders"
        },
        true,
        None::<&str>,
    )?;
    let dashboard = MenuItem::with_id(
        app,
        "dashboard",
        "Dashboard & Settings…",
        true,
        None::<&str>,
    )?;
    let cadence = MenuItem::with_id(
        app,
        "cadence",
        format!("Every {interval} minutes"),
        false,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit PUMZI", true, None::<&str>)?;
    let first_sep = PredefinedMenuItem::separator(app)?;
    let second_sep = PredefinedMenuItem::separator(app)?;
    Menu::with_items(
        app,
        &[
            &heading,
            &first_sep,
            &take,
            &pause,
            &dashboard,
            &second_sep,
            &cadence,
            &quit,
        ],
    )
}

fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        "take" => show_break(app),
        "pause" => toggle_pause_now(app),
        "dashboard" => open_dashboard_window(app),
        "quit" => quit(app),
        _ => {}
    }
}

fn menu_label(status: &Status) -> String {
    match status.state.as_str() {
        "paused" => "Paused".into(),
        "away" => "Away from keyboard".into(),
        "quiet" => "Quiet hours".into(),
        "focus" => format!("Focus — {} left", format_duration(status.ms.unwrap_or(0))),
        "break" => format!("Break — {} left", format_duration(status.ms.unwrap_or(0))),
        _ => {
            let minutes = ((status.ms.unwrap_or(0) + 59_999) / 60_000).max(1);
            if minutes <= 1 {
                "Micro-break ready".into()
            } else {
                format!("Micro-break in {minutes} min")
            }
        }
    }
}

fn update_tray(app: &AppHandle) {
    let now = now_ms();
    let idle = idle_seconds();
    let (status, paused, key, title, tooltip, interval, last_title) = {
        let state = app.state::<Mutex<AppState>>();
        let app_state = state.lock().unwrap();
        let status = live_status(&app_state, now, idle);
        let paused = is_paused(&app_state);
        let label = menu_label(&status);
        let bucket = status.ms.unwrap_or(0) / 60_000;
        let interval = app_state.store.config.interval_minutes;
        let key = format!(
            "{}|{}|{}|{}|{}|{}",
            paused, status.state, bucket, interval, app_state.card_open, label
        );
        let title = if paused {
            " –".to_string()
        } else if matches!(status.state.as_str(), "focus" | "break") {
            format!(" {}", format_duration(status.ms.unwrap_or(0)))
        } else {
            String::new()
        };
        let tooltip = if paused {
            "PUMZI — paused".to_string()
        } else if status.state == "running" {
            format!(
                "PUMZI — micro-break in {}",
                format_duration(status.ms.unwrap_or(0))
            )
        } else if status.state == "focus" || status.state == "break" {
            format!(
                "PUMZI — {}, {} left",
                status.state,
                format_duration(status.ms.unwrap_or(0))
            )
        } else {
            format!("PUMZI — {}", status.label)
        };
        let last_title = app_state.last_tray_title.clone();
        (status, paused, key, title, tooltip, interval, last_title)
    };

    let Some(tray) = app.tray_by_id("main") else {
        return;
    };

    if title != last_title {
        #[cfg(target_os = "macos")]
        {
            let _ = tray.set_title(if title.is_empty() {
                None
            } else {
                Some(title.as_str())
            });
        }
        let state = app.state::<Mutex<AppState>>();
        let mut app_state = state.lock().unwrap();
        app_state.last_tray_title = title;
    }
    let _ = tray.set_tooltip(Some(tooltip.as_str()));

    let needs_menu = {
        let state = app.state::<Mutex<AppState>>();
        let mut app_state = state.lock().unwrap();
        if app_state.last_menu_key != key {
            app_state.last_menu_key = key;
            true
        } else {
            false
        }
    };
    if needs_menu {
        if let Ok(menu) = build_menu(app, paused, &menu_label(&status), interval) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

// ---- Scheduling ----

fn start_ticker(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(StdDuration::from_millis(1200));
        loop {
            tick(&app);
            std::thread::sleep(StdDuration::from_secs(1));
        }
    });
}

fn tick(app: &AppHandle) {
    let now = now_ms();
    let idle = idle_seconds();
    let mut show = false;
    let mut focus_message: Option<String> = None;
    let expired;

    {
        let state = app.state::<Mutex<AppState>>();
        let mut app_state = state.lock().unwrap();
        expired = app_state
            .safety_deadline
            .map(|deadline| Instant::now() >= deadline)
            .unwrap_or(false);

        let had_focus = app_state.store.tasks.active.is_some();
        let event = if let Some(focus) = app_state.store.tasks.active.as_mut() {
            focus.tick(now)
        } else {
            FocusEvent::None
        };

        if had_focus {
            if !matches!(event, FocusEvent::None) {
                app_state.store.save_tasks();
            }
            match event {
                FocusEvent::BreakStarted { .. } => {
                    show = true;
                }
                FocusEvent::FocusStarted => {
                    let title = app_state
                        .store
                        .tasks
                        .active
                        .as_ref()
                        .map(|focus| focus.title.clone())
                        .unwrap_or_default();
                    focus_message = Some(format!("Break over — back to “{title}”."));
                }
                FocusEvent::TaskCompleted => {
                    let title = app_state
                        .store
                        .tasks
                        .active
                        .as_ref()
                        .map(|focus| focus.title.clone())
                        .unwrap_or_default();
                    app_state.store.finish_active_task();
                    app_state.scheduler.reset(now);
                    focus_message = Some(format!("Task complete: “{title}”. Take a longer break."));
                    show = true;
                }
                FocusEvent::None => {}
            }
        } else {
            let due = app_state.scheduler.tick(now, idle);
            if due {
                show = true;
            }
        }
    }

    if expired {
        let state = app.state::<Mutex<AppState>>();
        let mut app_state = state.lock().unwrap();
        if app_state.card_open {
            hide_card(app, &mut app_state);
        }
    }

    if let Some(message) = focus_message {
        notify("PUMZI", &message);
    }

    if show {
        show_break(app);
    }

    update_tray(app);
}

fn live_status(app_state: &AppState, now: i64, idle: u64) -> Status {
    if let Some(focus) = &app_state.store.tasks.active {
        focus.status(now)
    } else {
        app_state.scheduler.status(now, idle)
    }
}

fn is_paused(app_state: &AppState) -> bool {
    if let Some(focus) = &app_state.store.tasks.active {
        focus.paused
    } else {
        app_state.scheduler.paused
    }
}

fn show_break(app: &AppHandle) {
    let now = Local::now();
    let (payload, position, height, break_name, sound) = {
        let state = app.state::<Mutex<AppState>>();
        let mut app_state = state.lock().unwrap();
        if app_state.card_open {
            return;
        }
        if !app_state
            .rotation
            .has_any(&app_state.store.config.categories)
        {
            drop(app_state);
            open_dashboard_window(app);
            return;
        }
        let enabled = app_state.store.config.categories.clone();
        let Some(item) = app_state.rotation.next(&enabled) else {
            return;
        };
        let category = app_state
            .breaks
            .categories
            .get(&item.category)
            .cloned()
            .unwrap_or_else(|| breaks::CategoryMeta {
                name: item.category.clone(),
                blurb: String::new(),
                color: "#D97757".into(),
            });
        let figure = breaks::figure_svg(&item.figure);
        let done_today = app_state.store.stats.summary(now).today_breaks;
        let payload = json!({
            "break": &item,
            "category": category,
            "figureSVG": figure,
            "doneToday": done_today,
            "snoozeMinutes": app_state.store.config.snooze_minutes,
            "expandSteps": app_state.store.config.expand_steps,
            "sound": app_state.store.config.sound,
            "position": app_state.store.config.position,
        });
        let position = app_state.store.config.position.clone();
        let height = app_state.card_height;
        let break_name = item.name.clone();
        let sound = app_state.store.config.sound;

        app_state.current = Some(item);
        app_state.card_open = true;
        app_state.pending_scheduled = false;
        app_state.safety_deadline = None;
        (payload, position, height, break_name, sound)
    };

    let Some(window) = app.get_webview_window("card") else {
        return;
    };
    let _ = window.emit("break:show", payload);
    position_card(app, &window, height, &position);
    let _ = window.show();
    if sound {
        play_alarm();
    }
    notify("PUMZI", &format!("Time for a break: {break_name}"));
    update_tray(app);
}

fn hide_card(app: &AppHandle, app_state: &mut AppState) {
    if let Some(window) = app.get_webview_window("card") {
        let _ = window.hide();
    }
    if app_state.card_open && !app_state.pending_scheduled {
        app_state.scheduler.reset(now_ms());
    }
    app_state.card_open = false;
    app_state.current = None;
    app_state.pending_scheduled = false;
    app_state.safety_deadline = None;
}

fn position_card(app: &AppHandle, window: &WebviewWindow, height: f64, position: &str) {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let width = (CARD_WIDTH * scale).round() as u32;
    let height = (height * scale).round() as u32;
    let inset = (CARD_INSET * scale).round() as i32;
    let right = position.ends_with("right");
    let bottom = position.starts_with("bottom");
    let x = if right {
        area.position.x + area.size.width as i32 - width as i32 - inset
    } else {
        area.position.x + inset
    };
    let y = if bottom {
        area.position.y + area.size.height as i32 - height as i32 - inset
    } else {
        area.position.y + inset
    };
    let _ = window.set_size(tauri::PhysicalSize::new(width, height));
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));

    let state = app.state::<Mutex<AppState>>();
    let mut app_state = state.lock().unwrap();
    app_state.card_height = height as f64 / scale;
}

// ---- Snapshot ----

fn now_ms() -> i64 {
    Local::now().timestamp_millis()
}

fn snapshot(app_state: &AppState) -> Value {
    let now = Local::now();
    let stats: Summary = app_state.store.stats.summary(now);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for item in &app_state.breaks.breaks {
        *counts.entry(item.category.clone()).or_insert(0) += 1;
    }
    json!({
        "config": &app_state.store.config,
        "paused": is_paused(app_state),
        "status": live_status(app_state, now_ms(), idle_seconds()),
        "stats": stats,
        "categories": &app_state.breaks.categories,
        "counts": counts,
        "tasks": &app_state.store.tasks.tasks,
        "focus": &app_state.store.tasks.active,
        "version": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "cardOpen": app_state.card_open,
    })
}

fn broadcast(app: &AppHandle) {
    let state = app.state::<Mutex<AppState>>();
    let app_state = state.lock().unwrap();
    let snapshot = snapshot(&app_state);
    drop(app_state);
    let _ = app.emit("state:changed", snapshot);
}

// ---- Actions ----

fn apply_launch_at_login(enabled: bool) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let path = exe.to_string_lossy().to_string();
    let mut builder = AutoLaunchBuilder::new();
    builder.set_app_name("PUMZI").set_app_path(&path);
    #[cfg(target_os = "macos")]
    builder.set_use_launch_agent(true);
    match builder.build() {
        Ok(auto) => {
            let result = if enabled {
                auto.enable()
            } else {
                auto.disable()
            };
            if let Err(error) = result {
                eprintln!("Could not update login item: {error}");
            }
        }
        Err(error) => eprintln!("Could not configure login item: {error}"),
    }
}

fn toggle_pause_now(app: &AppHandle) {
    let now = now_ms();
    {
        let state = app.state::<Mutex<AppState>>();
        let mut app_state = state.lock().unwrap();
        if let Some(focus) = app_state.store.tasks.active.as_mut() {
            focus.toggle_pause(now);
        }
        app_state.scheduler.toggle(now);
        app_state.store.save_tasks();
    }
    update_tray(app);
    broadcast(app);
}

fn open_dashboard_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(window) = app.get_webview_window("dashboard") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    broadcast(app);
}

fn close_dashboard_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("dashboard") {
        let _ = window.hide();
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

fn quit(app: &AppHandle) {
    QUITTING.store(true, Ordering::SeqCst);
    app.exit(0);
}

// ---- Commands ----

#[tauri::command]
fn app_state(state: State<'_, Mutex<AppState>>) -> Value {
    let app_state = state.lock().unwrap();
    snapshot(&app_state)
}

#[tauri::command]
fn update_config(app: AppHandle, state: State<'_, Mutex<AppState>>, patch: Value) -> Value {
    let (result, launch) = {
        let mut app_state = state.lock().unwrap();
        let config = app_state.store.update_config(&patch);
        app_state.scheduler.set_config(&config, now_ms());
        (snapshot(&app_state), config.launch_at_login)
    };
    if patch.get("launchAtLogin").is_some() {
        apply_launch_at_login(launch);
    }
    update_tray(&app);
    let _ = app.emit("state:changed", result.clone());
    result
}

#[tauri::command]
fn card_action(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, Mutex<AppState>>,
    action: String,
    seconds: Option<f64>,
) -> Value {
    if window.label() != "card" {
        return json!({ "ok": false });
    }

    let mut result = json!({ "ok": true });
    let snapshot_value = {
        let mut app_state = state.lock().unwrap();
        match action.as_str() {
            "done" => {
                let item = app_state.current.clone();
                let fallback = item
                    .as_ref()
                    .map(|item| item.duration_seconds as f64)
                    .unwrap_or(1.0);
                let seconds = seconds.unwrap_or(fallback).clamp(1.0, 3600.0).round() as u32;
                if let Some(item) = &item {
                    app_state.store.record_break(item, seconds, Local::now());
                }
                app_state.scheduler.taken(now_ms());
                app_state.pending_scheduled = true;
                app_state.safety_deadline = Some(Instant::now() + StdDuration::from_secs(5));
                let done = app_state.store.stats.summary(Local::now()).today_breaks;
                result["doneToday"] = json!(done);
            }
            "snooze" => {
                app_state.scheduler.snooze(now_ms());
                app_state.pending_scheduled = true;
                app_state.safety_deadline = Some(Instant::now() + StdDuration::from_secs(5));
            }
            "skip" | "ignored" => {
                app_state.scheduler.reset(now_ms());
                app_state.pending_scheduled = true;
                app_state.safety_deadline = Some(Instant::now() + StdDuration::from_secs(5));
            }
            "closed" => {
                hide_card(&app, &mut app_state);
            }
            _ => {}
        }
        snapshot(&app_state)
    };

    let _ = app.emit("state:changed", snapshot_value);
    update_tray(&app);
    result
}

#[tauri::command]
fn resize_card(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, Mutex<AppState>>,
    height: f64,
) -> Value {
    if window.label() != "card" {
        return json!({ "ok": false });
    }
    let position = {
        let app_state = state.lock().unwrap();
        app_state.store.config.position.clone()
    };
    let height = height.clamp(120.0, 900.0);
    if let Some(card) = app.get_webview_window("card") {
        position_card(&app, &card, height, &position);
    }
    json!({ "ok": true })
}

#[tauri::command]
fn take_break(app: AppHandle, state: State<'_, Mutex<AppState>>) -> Value {
    let open = {
        let app_state = state.lock().unwrap();
        app_state.card_open
    };
    if !open {
        show_break(&app);
    }
    json!({ "ok": true })
}

#[tauri::command]
fn toggle_pause(app: AppHandle) -> Value {
    toggle_pause_now(&app);
    app_state(app.state::<Mutex<AppState>>())
}

#[tauri::command]
fn open_dashboard(app: AppHandle) -> Value {
    open_dashboard_window(&app);
    json!({ "ok": true })
}

#[tauri::command]
fn close_dashboard(app: AppHandle) -> Value {
    close_dashboard_window(&app);
    json!({ "ok": true })
}

#[tauri::command]
fn add_task(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    title: String,
    minutes: u32,
) -> Value {
    let result = {
        let mut app_state = state.lock().unwrap();
        app_state.store.add_task(&title, minutes);
        snapshot(&app_state)
    };
    update_tray(&app);
    let _ = app.emit("state:changed", result.clone());
    result
}

#[tauri::command]
fn start_task(app: AppHandle, state: State<'_, Mutex<AppState>>, id: String) -> Value {
    let result = {
        let mut app_state = state.lock().unwrap();
        if app_state.store.tasks.active.is_none() {
            if app_state.card_open {
                hide_card(&app, &mut app_state);
            }
            if app_state.store.start_task(&id, now_ms()) {
                app_state.scheduler.resume(now_ms());
            }
        }
        snapshot(&app_state)
    };
    update_tray(&app);
    let _ = app.emit("state:changed", result.clone());
    result
}

#[tauri::command]
fn stop_task(app: AppHandle, state: State<'_, Mutex<AppState>>) -> Value {
    let result = {
        let mut app_state = state.lock().unwrap();
        app_state.store.stop_task();
        app_state.scheduler.reset(now_ms());
        if app_state.card_open {
            hide_card(&app, &mut app_state);
        }
        snapshot(&app_state)
    };
    update_tray(&app);
    let _ = app.emit("state:changed", result.clone());
    result
}

#[tauri::command]
fn delete_task(app: AppHandle, state: State<'_, Mutex<AppState>>, id: String) -> Value {
    let result = {
        let mut app_state = state.lock().unwrap();
        app_state.store.delete_task(&id);
        snapshot(&app_state)
    };
    update_tray(&app);
    let _ = app.emit("state:changed", result.clone());
    result
}

#[tauri::command]
fn reset_stats(app: AppHandle, state: State<'_, Mutex<AppState>>) -> Value {
    let result = {
        let mut app_state = state.lock().unwrap();
        app_state.store.reset_stats();
        snapshot(&app_state)
    };
    let _ = app.emit("state:changed", result.clone());
    update_tray(&app);
    result
}

#[tauri::command]
fn quit_app(app: AppHandle) -> Value {
    quit(&app);
    json!({ "ok": true })
}

fn play_alarm() {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("afplay")
            .arg("/System/Library/Sounds/Glass.aiff")
            .spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-Command", "[console]::beep(880,450)"])
            .spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("paplay")
            .arg("/usr/share/sounds/freedesktop/stereo/complete.oga")
            .spawn();
    }
}

fn notify(title: &str, body: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("osascript")
            .args([
                "-e",
                "on run argv",
                "-e",
                "display notification (item 2 of argv) with title (item 1 of argv)",
                "-e",
                "end run",
                title,
                body,
            ])
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("notify-send").args([title, body]).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = (title, body);
    }
}

#[cfg(target_os = "macos")]
fn idle_seconds() -> u64 {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    }
    const COMBINED_SESSION_STATE: i32 = 0;
    const ANY_INPUT_EVENT_TYPE: u32 = 0xFFFF_FFFF;
    unsafe {
        CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT_TYPE)
            .max(0.0) as u64
    }
}

#[cfg(not(target_os = "macos"))]
fn idle_seconds() -> u64 {
    0
}
