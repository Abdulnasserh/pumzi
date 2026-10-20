use chrono::{DateTime, Duration, Local, TimeZone, Timelike};
use serde::Serialize;

use crate::config::{Config, QuietHours};

const MINUTE: i64 = 60_000;

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub state: String,
    pub label: String,
    pub ms: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct Scheduler {
    pub paused: bool,
    pub away: bool,
    pub next_at: i64,
    config: Config,
}

impl Scheduler {
    pub fn new(config: &Config, now: i64) -> Self {
        let mut scheduler = Self {
            paused: false,
            away: false,
            next_at: now,
            config: config.clone(),
        };
        scheduler.next_at = now + scheduler.interval_ms();
        scheduler
    }

    pub fn interval_ms(&self) -> i64 {
        self.config.interval_minutes.max(1) as i64 * MINUTE
    }

    pub fn snooze_ms(&self) -> i64 {
        self.config.snooze_minutes.max(1) as i64 * MINUTE
    }

    pub fn set_config(&mut self, config: &Config, now: i64) {
        let previous = self.interval_ms();
        self.config = config.clone();
        if self.interval_ms() != previous {
            let remaining = (self.next_at - now).max(0);
            self.next_at = now + remaining.min(self.interval_ms());
        }
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self, now: i64) {
        self.paused = false;
        self.next_at = now + self.interval_ms();
    }

    pub fn toggle(&mut self, now: i64) -> bool {
        if self.paused {
            self.resume(now);
        } else {
            self.pause();
        }
        self.paused
    }

    pub fn reset(&mut self, now: i64) {
        self.next_at = now + self.interval_ms();
    }

    pub fn snooze(&mut self, now: i64) {
        self.next_at = now + self.snooze_ms();
    }

    pub fn taken(&mut self, now: i64) {
        self.reset(now);
    }

    pub fn quiet(&self, now: i64) -> bool {
        in_quiet_hours(local(now), &self.config.quiet_hours)
    }

    /// Advances idle/quiet state. Returns true when a card should be shown now.
    pub fn tick(&mut self, now: i64, idle_seconds: u64) -> bool {
        if self.config.reset_when_away {
            if idle_seconds >= 300 {
                if !self.away {
                    self.away = true;
                    self.next_at = now + self.interval_ms();
                }
                return false;
            }
            if self.away && idle_seconds < 20 {
                self.away = false;
                self.next_at = now + self.interval_ms();
            }
        } else {
            self.away = false;
        }

        if self.quiet(now) {
            let end = quiet_end_ms(local(now), &self.config.quiet_hours);
            if self.next_at < end {
                self.next_at = end;
            }
            return false;
        }

        let hold_for_idle = idle_seconds >= 90;
        !self.paused && !self.away && !hold_for_idle && now >= self.next_at
    }

    pub fn status(&self, now: i64, _idle_seconds: u64) -> Status {
        if self.paused {
            return Status {
                state: "paused".into(),
                label: "Paused".into(),
                ms: None,
            };
        }
        if self.away {
            return Status {
                state: "away".into(),
                label: "Away".into(),
                ms: None,
            };
        }
        if self.quiet(now) {
            return Status {
                state: "quiet".into(),
                label: "Quiet hours".into(),
                ms: Some(self.next_at - now),
            };
        }
        let ms = (self.next_at - now).max(0);
        let label = if ms < 60_000 {
            "Due now".to_string()
        } else {
            format!("Next break in {}", format_duration(ms))
        };
        Status {
            state: "running".into(),
            label,
            ms: Some(ms),
        }
    }
}

pub fn local(now: i64) -> DateTime<Local> {
    Local
        .timestamp_millis_opt(now)
        .single()
        .unwrap_or_else(Local::now)
}

pub fn parse_clock(value: &str) -> Option<(u32, u32)> {
    let mut parts = value.split(':');
    let (Some(hours), Some(minutes), None) = (parts.next(), parts.next(), parts.next()) else {
        return None;
    };
    let hours = hours.parse::<u32>().ok()?;
    let minutes = minutes.parse::<u32>().ok()?;
    if hours > 23 || minutes > 59 {
        return None;
    }
    Some((hours, minutes))
}

pub fn in_quiet_hours(now: DateTime<Local>, quiet: &QuietHours) -> bool {
    if !quiet.enabled {
        return false;
    }
    let Some((start_h, start_m)) = parse_clock(&quiet.start) else {
        return false;
    };
    let Some((end_h, end_m)) = parse_clock(&quiet.end) else {
        return false;
    };
    let start = start_h * 60 + start_m;
    let end = end_h * 60 + end_m;
    if start == end {
        return false;
    }
    let now_minutes = now.hour() * 60 + now.minute();
    if start < end {
        now_minutes >= start && now_minutes < end
    } else {
        now_minutes >= start || now_minutes < end
    }
}

pub fn quiet_end_ms(now: DateTime<Local>, quiet: &QuietHours) -> i64 {
    if !in_quiet_hours(now, quiet) {
        return now.timestamp_millis();
    }
    let Some((end_h, end_m)) = parse_clock(&quiet.end) else {
        return now.timestamp_millis();
    };
    let Some(naive) = now.date_naive().and_hms_opt(end_h, end_m, 0) else {
        return now.timestamp_millis();
    };
    let mut end = Local.from_local_datetime(&naive).earliest().unwrap_or(now);
    if end.timestamp_millis() <= now.timestamp_millis() {
        end += Duration::days(1);
    }
    end.timestamp_millis()
}

pub fn format_duration(ms: i64) -> String {
    let total = (ms.max(0) + 500) / 1000;
    let minutes = total / 60;
    let seconds = total % 60;
    if minutes >= 60 {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: u32, hour: u32, minute: u32) -> i64 {
        Local
            .with_ymd_and_hms(2026, 10, day, hour, minute, 0)
            .single()
            .unwrap()
            .timestamp_millis()
    }

    fn config() -> Config {
        Config::default()
    }

    #[test]
    fn first_break_is_due_after_one_interval() {
        let now = at(20, 9, 0);
        let mut scheduler = Scheduler::new(&config(), now);
        assert!(!scheduler.tick(now + 19 * MINUTE, 0));
        assert!(scheduler.tick(now + 20 * MINUTE, 0));
    }

    #[test]
    fn snooze_pushes_the_next_break_out() {
        let now = at(20, 9, 0);
        let mut scheduler = Scheduler::new(&config(), now);
        scheduler.snooze(now);
        assert!(!scheduler.tick(now + 9 * MINUTE, 0));
        assert!(scheduler.tick(now + 10 * MINUTE, 0));
    }

    #[test]
    fn pause_holds_and_resume_restarts_the_interval() {
        let now = at(20, 9, 0);
        let mut scheduler = Scheduler::new(&config(), now);
        scheduler.pause();
        assert!(!scheduler.tick(now + 60 * MINUTE, 0));
        scheduler.resume(now + 60 * MINUTE);
        assert!(!scheduler.tick(now + 79 * MINUTE, 0));
        assert!(scheduler.tick(now + 80 * MINUTE, 0));
    }

    #[test]
    fn quiet_hours_suppress_cards_and_push_to_the_end() {
        let mut cfg = config();
        cfg.quiet_hours.enabled = true;
        let late = at(20, 23, 30);
        let mut scheduler = Scheduler::new(&cfg, late);
        assert!(scheduler.quiet(late));
        assert!(!scheduler.tick(late + 30 * MINUTE, 0));
        let end = quiet_end_ms(local(late), &cfg.quiet_hours);
        assert_eq!(scheduler.next_at, end);
        assert!(!scheduler.quiet(end));
    }

    #[test]
    fn quiet_hours_work_across_midnight() {
        let quiet = QuietHours {
            enabled: true,
            start: "22:00".into(),
            end: "07:00".into(),
        };
        assert!(in_quiet_hours(local(at(21, 2, 0)), &quiet));
        assert!(!in_quiet_hours(local(at(21, 12, 0)), &quiet));
        assert!(in_quiet_hours(local(at(21, 6, 59)), &quiet));
        assert!(!in_quiet_hours(local(at(21, 7, 0)), &quiet));
    }

    #[test]
    fn away_time_resets_and_return_restarts_the_clock() {
        let now = at(20, 9, 0);
        let mut scheduler = Scheduler::new(&config(), now);
        scheduler.tick(now + 15 * MINUTE, 10);
        assert!(!scheduler.away);
        scheduler.tick(now + 20 * MINUTE, 305);
        assert!(scheduler.away);
        let return_at = now + 30 * MINUTE;
        scheduler.tick(return_at, 2);
        assert!(!scheduler.away);
        assert_eq!(scheduler.next_at, return_at + 20 * MINUTE);
    }

    #[test]
    fn a_due_break_is_held_while_briefly_idle() {
        let now = at(20, 9, 0);
        let mut scheduler = Scheduler::new(&config(), now);
        assert!(!scheduler.tick(now + 20 * MINUTE, 120));
        assert!(scheduler.tick(now + 20 * MINUTE, 5));
    }

    #[test]
    fn changing_interval_clamps_the_wait() {
        let now = at(20, 9, 0);
        let mut scheduler = Scheduler::new(&config(), now);
        let mut cfg = config();
        cfg.interval_minutes = 5;
        scheduler.set_config(&cfg, now + 4 * MINUTE);
        assert_eq!(scheduler.next_at, now + 9 * MINUTE);
    }

    #[test]
    fn status_labels_are_readable() {
        let now = at(20, 9, 0);
        let mut scheduler = Scheduler::new(&config(), now);
        assert_eq!(scheduler.status(now, 0).state, "running");
        scheduler.pause();
        assert_eq!(scheduler.status(now, 0).state, "paused");
        assert_eq!(format_duration(65_000), "1:05");
        assert_eq!(format_duration(65 * MINUTE), "1h 05m");
    }
}
