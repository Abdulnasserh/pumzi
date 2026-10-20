use serde::{Deserialize, Serialize};

use crate::scheduler::{format_duration, Status};

pub const SESSION_SECONDS: i64 = 25 * 60;
pub const SHORT_BREAK_SECONDS: i64 = 5 * 60;
pub const LONG_BREAK_SECONDS: i64 = 15 * 60;
pub const SESSIONS_BEFORE_LONG: u32 = 4;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub minutes: u32,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusState {
    pub task_id: String,
    pub title: String,
    pub phase: String,
    pub phase_started_at: i64,
    pub phase_ends_at: i64,
    pub remaining_focus_seconds: i64,
    pub session_seconds: i64,
    pub break_seconds: i64,
    pub long_break_seconds: i64,
    pub sessions_before_long: u32,
    pub completed_sessions: u32,
    pub total_sessions: u32,
    pub total_minutes: u32,
    pub paused: bool,
    pub paused_at: Option<i64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TasksFile {
    pub tasks: Vec<Task>,
    pub active: Option<FocusState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusEvent {
    None,
    BreakStarted { long: bool },
    FocusStarted,
    TaskCompleted,
}

impl FocusState {
    pub fn start(task: &Task, now: i64) -> Self {
        let total_seconds = (task.minutes.max(1) as i64) * 60;
        let session_seconds = total_seconds.min(SESSION_SECONDS);
        let total_sessions = ((total_seconds + SESSION_SECONDS - 1) / SESSION_SECONDS) as u32;
        Self {
            task_id: task.id.clone(),
            title: task.title.clone(),
            phase: "focus".into(),
            phase_started_at: now,
            phase_ends_at: now + session_seconds * 1000,
            remaining_focus_seconds: total_seconds,
            session_seconds: SESSION_SECONDS,
            break_seconds: SHORT_BREAK_SECONDS,
            long_break_seconds: LONG_BREAK_SECONDS,
            sessions_before_long: SESSIONS_BEFORE_LONG,
            completed_sessions: 0,
            total_sessions,
            total_minutes: task.minutes,
            paused: false,
            paused_at: None,
        }
    }

    pub fn tick(&mut self, now: i64) -> FocusEvent {
        if self.paused || now < self.phase_ends_at {
            return FocusEvent::None;
        }

        if self.phase == "focus" {
            let focused = ((self.phase_ends_at - self.phase_started_at) / 1000).max(0);
            self.remaining_focus_seconds = (self.remaining_focus_seconds - focused).max(0);
            self.completed_sessions += 1;

            if self.remaining_focus_seconds == 0 {
                return FocusEvent::TaskCompleted;
            }

            let long = self.sessions_before_long > 0
                && self.completed_sessions % self.sessions_before_long == 0;
            let break_seconds = if long {
                self.long_break_seconds
            } else {
                self.break_seconds
            };
            self.phase = "break".into();
            self.phase_started_at = now;
            self.phase_ends_at = now + break_seconds * 1000;
            FocusEvent::BreakStarted { long }
        } else {
            let session_seconds = self
                .remaining_focus_seconds
                .min(self.session_seconds)
                .max(60);
            self.phase = "focus".into();
            self.phase_started_at = now;
            self.phase_ends_at = now + session_seconds * 1000;
            FocusEvent::FocusStarted
        }
    }

    pub fn pause(&mut self, now: i64) {
        if self.paused {
            return;
        }
        self.paused = true;
        self.paused_at = Some(now);
    }

    pub fn resume(&mut self, now: i64) {
        if !self.paused {
            return;
        }
        if let Some(paused_at) = self.paused_at.take() {
            self.phase_ends_at += (now - paused_at).max(0);
        }
        self.paused = false;
    }

    pub fn toggle_pause(&mut self, now: i64) -> bool {
        if self.paused {
            self.resume(now);
        } else {
            self.pause(now);
        }
        self.paused
    }

    pub fn status(&self, now: i64) -> Status {
        let ms = if self.paused {
            (self.phase_ends_at - self.paused_at.unwrap_or(now)).max(0)
        } else {
            (self.phase_ends_at - now).max(0)
        };
        if self.paused {
            return Status {
                state: "paused".into(),
                label: "Task paused".into(),
                ms: Some(ms),
            };
        }
        if self.phase == "focus" {
            Status {
                state: "focus".into(),
                label: format!("Focus · {}", format_duration(ms)),
                ms: Some(ms),
            }
        } else {
            Status {
                state: "break".into(),
                label: format!("Break · {}", format_duration(ms)),
                ms: Some(ms),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(minutes: u32) -> Task {
        Task {
            id: "t1".into(),
            title: "Write the thing".into(),
            minutes,
            created_at: String::new(),
            completed_at: None,
        }
    }

    #[test]
    fn a_short_task_starts_with_a_focus_session() {
        let focus = FocusState::start(&task(20), 0);
        assert_eq!(focus.phase, "focus");
        assert_eq!(focus.phase_ends_at, 20 * 60 * 1000);
        assert_eq!(focus.total_sessions, 1);
        assert_eq!(focus.remaining_focus_seconds, 20 * 60);
    }

    #[test]
    fn focus_ends_in_a_short_break_then_resumes() {
        let mut focus = FocusState::start(&task(50), 0);
        assert_eq!(
            focus.tick(25 * 60 * 1000),
            FocusEvent::BreakStarted { long: false }
        );
        assert_eq!(focus.phase, "break");
        assert_eq!(focus.completed_sessions, 1);
        assert_eq!(focus.remaining_focus_seconds, 25 * 60);
        assert_eq!(focus.tick(30 * 60 * 1000), FocusEvent::FocusStarted);
        assert_eq!(focus.phase, "focus");
        assert_eq!(focus.phase_ends_at, 55 * 60 * 1000);
    }

    #[test]
    fn the_final_focus_completes_the_task() {
        let mut focus = FocusState::start(&task(25), 0);
        assert_eq!(focus.tick(25 * 60 * 1000), FocusEvent::TaskCompleted);
    }

    #[test]
    fn a_long_break_follows_the_fourth_session() {
        let mut focus = FocusState::start(&task(120), 0);
        assert_eq!(
            focus.tick(25 * 60 * 1000),
            FocusEvent::BreakStarted { long: false }
        );
        assert_eq!(focus.tick(30 * 60 * 1000), FocusEvent::FocusStarted);
        assert_eq!(
            focus.tick(55 * 60 * 1000),
            FocusEvent::BreakStarted { long: false }
        );
        assert_eq!(focus.tick(60 * 60 * 1000), FocusEvent::FocusStarted);
        assert_eq!(
            focus.tick(85 * 60 * 1000),
            FocusEvent::BreakStarted { long: false }
        );
        assert_eq!(focus.tick(90 * 60 * 1000), FocusEvent::FocusStarted);
        assert_eq!(
            focus.tick(115 * 60 * 1000),
            FocusEvent::BreakStarted { long: true }
        );
    }

    #[test]
    fn pausing_freezes_the_countdown() {
        let mut focus = FocusState::start(&task(25), 0);
        focus.pause(5 * 60 * 1000);
        assert_eq!(focus.tick(30 * 60 * 1000), FocusEvent::None);
        assert_eq!(focus.status(30 * 60 * 1000).ms, Some(20 * 60 * 1000));
        focus.resume(30 * 60 * 1000);
        assert_eq!(focus.phase_ends_at, 50 * 60 * 1000);
        assert_eq!(focus.status(50 * 60 * 1000).state, "focus");
    }

    #[test]
    fn total_sessions_rounds_up() {
        assert_eq!(FocusState::start(&task(30), 0).total_sessions, 2);
        assert_eq!(FocusState::start(&task(51), 0).total_sessions, 3);
    }
}
