use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::breaks::BreakItem;
use crate::config::{merge_values, Config};
use crate::stats::StatsFile;
use crate::tasks::{FocusState, Task, TasksFile};

pub struct Store {
    dir: PathBuf,
    pub config: Config,
    pub stats: StatsFile,
    pub tasks: TasksFile,
}

impl Store {
    pub fn load(dir: PathBuf) -> Self {
        let config: Config = read_json::<Config>(&dir.join("config.json"))
            .unwrap_or_default()
            .sanitized();
        let stats: StatsFile = read_json(&dir.join("stats.json")).unwrap_or_default();
        let tasks: TasksFile = read_json(&dir.join("tasks.json")).unwrap_or_default();
        Self {
            dir,
            config,
            stats,
            tasks,
        }
    }

    pub fn update_config(&mut self, patch: &Value) -> Config {
        let mut base = serde_json::to_value(&self.config)
            .unwrap_or_else(|_| Value::Object(Default::default()));
        merge_values(&mut base, patch);
        let config: Config = serde_json::from_value::<Config>(base)
            .unwrap_or_default()
            .sanitized();
        self.config = config;
        self.save_config();
        self.config.clone()
    }

    pub fn record_break(&mut self, item: &BreakItem, seconds: u32, now: DateTime<Local>) {
        self.stats
            .record(&item.id, &item.name, &item.category, seconds, now);
        self.save_stats();
    }

    pub fn reset_stats(&mut self) {
        self.stats = StatsFile::default();
        self.save_stats();
    }

    pub fn save_config(&self) {
        write_json(&self.dir.join("config.json"), &self.config);
    }

    pub fn save_stats(&self) {
        write_json(&self.dir.join("stats.json"), &self.stats);
    }

    pub fn save_tasks(&self) {
        write_json(&self.dir.join("tasks.json"), &self.tasks);
    }

    pub fn add_task(&mut self, title: &str, minutes: u32) -> bool {
        let title: String = title.trim().chars().take(80).collect();
        if title.is_empty() {
            return false;
        }
        let now = Local::now();
        let id = format!("task-{}-{}", now.timestamp_millis(), self.tasks.tasks.len());
        self.tasks.tasks.insert(
            0,
            Task {
                id,
                title,
                minutes: minutes.clamp(5, 480),
                created_at: now.to_rfc3339(),
                completed_at: None,
            },
        );
        self.save_tasks();
        true
    }

    pub fn delete_task(&mut self, id: &str) -> bool {
        if self
            .tasks
            .active
            .as_ref()
            .map(|focus| focus.task_id == id)
            .unwrap_or(false)
        {
            return false;
        }
        let before = self.tasks.tasks.len();
        self.tasks.tasks.retain(|task| task.id != id);
        let removed = self.tasks.tasks.len() != before;
        if removed {
            self.save_tasks();
        }
        removed
    }

    pub fn start_task(&mut self, id: &str, now: i64) -> bool {
        if self.tasks.active.is_some() {
            return false;
        }
        let task = self
            .tasks
            .tasks
            .iter()
            .find(|task| task.id == id && task.completed_at.is_none())
            .cloned();
        let Some(task) = task else { return false };
        self.tasks.active = Some(FocusState::start(&task, now));
        self.save_tasks();
        true
    }

    pub fn stop_task(&mut self) {
        if self.tasks.active.take().is_some() {
            self.save_tasks();
        }
    }

    pub fn finish_active_task(&mut self) -> bool {
        let Some(focus) = self.tasks.active.take() else {
            return false;
        };
        if let Some(task) = self
            .tasks
            .tasks
            .iter_mut()
            .find(|task| task.id == focus.task_id)
        {
            task.completed_at = Some(Local::now().to_rfc3339());
        }
        self.save_tasks();
        true
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let Ok(text) = serde_json::to_string_pretty(value) else {
        return;
    };
    let tmp = path.with_extension("json.tmp");
    if fs::write(&tmp, format!("{text}\n")).is_ok() && fs::rename(&tmp, path).is_err() {
        let _ = fs::remove_file(path);
        if fs::rename(&tmp, path).is_err() {
            let _ = fs::copy(&tmp, path);
            let _ = fs::remove_file(&tmp);
        }
    }
}
