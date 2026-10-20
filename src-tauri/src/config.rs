use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const POSITIONS: [&str; 4] = ["bottom-right", "bottom-left", "top-right", "top-left"];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub interval_minutes: u32,
    pub snooze_minutes: u32,
    pub position: String,
    pub categories: Categories,
    pub quiet_hours: QuietHours,
    pub reset_when_away: bool,
    pub expand_steps: bool,
    pub sound: bool,
    pub launch_at_login: bool,
    pub first_run: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Categories {
    pub eyes: bool,
    pub hydration: bool,
    pub breathing: bool,
    pub movement: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct QuietHours {
    pub enabled: bool,
    pub start: String,
    pub end: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            interval_minutes: 20,
            snooze_minutes: 10,
            position: "bottom-right".into(),
            categories: Categories::default(),
            quiet_hours: QuietHours::default(),
            reset_when_away: true,
            expand_steps: false,
            sound: true,
            launch_at_login: false,
            first_run: true,
        }
    }
}

impl Default for Categories {
    fn default() -> Self {
        Self {
            eyes: true,
            hydration: true,
            breathing: true,
            movement: true,
        }
    }
}

impl Default for QuietHours {
    fn default() -> Self {
        Self {
            enabled: false,
            start: "22:00".into(),
            end: "07:00".into(),
        }
    }
}

impl Categories {
    pub fn enabled(&self, name: &str) -> bool {
        match name {
            "eyes" => self.eyes,
            "hydration" => self.hydration,
            "breathing" => self.breathing,
            "movement" => self.movement,
            _ => false,
        }
    }
}

impl Config {
    pub fn sanitized(self) -> Self {
        let mut config = self;
        config.interval_minutes = config.interval_minutes.clamp(5, 120);
        config.snooze_minutes = config.snooze_minutes.clamp(1, 60);
        if !POSITIONS.contains(&config.position.as_str()) {
            config.position = "bottom-right".into();
        }
        config.quiet_hours.start = clean_time(&config.quiet_hours.start, "22:00");
        config.quiet_hours.end = clean_time(&config.quiet_hours.end, "07:00");
        config
    }
}

fn clean_time(value: &str, fallback: &str) -> String {
    let mut parts = value.split(':');
    let (Some(hours), Some(minutes), None) = (parts.next(), parts.next(), parts.next()) else {
        return fallback.into();
    };
    let Ok(hours) = hours.parse::<u32>() else {
        return fallback.into();
    };
    let Ok(minutes) = minutes.parse::<u32>() else {
        return fallback.into();
    };
    if hours > 23 || minutes > 59 {
        return fallback.into();
    }
    format!("{hours:02}:{minutes:02}")
}

pub fn merge_values(base: &mut Value, patch: &Value) {
    if let (Some(base), Some(patch)) = (base.as_object_mut(), patch.as_object()) {
        for (key, value) in patch {
            match base.get_mut(key) {
                Some(existing) if existing.is_object() && value.is_object() => {
                    merge_values(existing, value);
                }
                _ => {
                    base.insert(key.clone(), value.clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sanitizes_ranges_and_positions() {
        let config = Config {
            interval_minutes: 1,
            snooze_minutes: 999,
            position: "middle".into(),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(config.interval_minutes, 5);
        assert_eq!(config.snooze_minutes, 60);
        assert_eq!(config.position, "bottom-right");
    }

    #[test]
    fn merges_partial_patches() {
        let mut config = Config::default();
        let mut value = serde_json::to_value(&config).unwrap();
        merge_values(
            &mut value,
            &json!({ "categories": { "eyes": false }, "quietHours": { "enabled": true } }),
        );
        config = serde_json::from_value(value).unwrap();
        assert!(!config.categories.eyes);
        assert!(config.categories.movement);
        assert!(config.quiet_hours.enabled);
        assert_eq!(config.quiet_hours.start, "22:00");
    }

    #[test]
    fn fixes_bad_times() {
        let config = Config {
            quiet_hours: QuietHours {
                enabled: true,
                start: "9:5".into(),
                end: "25:00".into(),
            },
            ..Config::default()
        }
        .sanitized();
        assert_eq!(config.quiet_hours.start, "09:05");
        assert_eq!(config.quiet_hours.end, "07:00");
    }
}
