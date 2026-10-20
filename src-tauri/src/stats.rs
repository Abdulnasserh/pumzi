use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Day {
    pub breaks: u32,
    pub seconds: u32,
    #[serde(default)]
    pub categories: BTreeMap<String, u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Totals {
    pub breaks: u32,
    pub seconds: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentBreak {
    pub at: String,
    pub id: String,
    pub name: String,
    pub category: String,
    pub seconds: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsFile {
    pub days: BTreeMap<String, Day>,
    pub totals: Totals,
    pub recent: Vec<RecentBreak>,
    pub first_break_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekDay {
    pub key: String,
    pub label: String,
    pub breaks: u32,
    pub seconds: u32,
    pub today: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub today_breaks: u32,
    pub today_seconds: u32,
    pub today_categories: BTreeMap<String, u32>,
    pub streak: u32,
    pub total_breaks: u32,
    pub total_seconds: u32,
    pub categories: BTreeMap<String, u32>,
    pub week: Vec<WeekDay>,
    pub recent: Vec<RecentBreak>,
}

pub fn day_key(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

impl StatsFile {
    pub fn record(
        &mut self,
        id: &str,
        name: &str,
        category: &str,
        seconds: u32,
        now: DateTime<Local>,
    ) {
        let key = day_key(now.date_naive());
        let day = self.days.entry(key).or_default();
        day.breaks += 1;
        day.seconds += seconds;
        *day.categories.entry(category.to_string()).or_insert(0) += 1;

        self.totals.breaks += 1;
        self.totals.seconds += seconds;
        if self.first_break_at.is_none() {
            self.first_break_at = Some(now.to_rfc3339());
        }
        self.recent.insert(
            0,
            RecentBreak {
                at: now.to_rfc3339(),
                id: id.to_string(),
                name: name.to_string(),
                category: category.to_string(),
                seconds,
            },
        );
        self.recent.truncate(100);
    }

    pub fn compute_streak(&self, today: NaiveDate) -> u32 {
        let mut cursor = today;
        if self
            .days
            .get(&day_key(cursor))
            .map(|d| d.breaks)
            .unwrap_or(0)
            == 0
        {
            cursor -= Duration::days(1);
        }
        let mut streak = 0;
        while self
            .days
            .get(&day_key(cursor))
            .map(|d| d.breaks)
            .unwrap_or(0)
            > 0
        {
            streak += 1;
            cursor -= Duration::days(1);
        }
        streak
    }

    pub fn last_seven(&self, today: NaiveDate) -> Vec<WeekDay> {
        let mut out = Vec::with_capacity(7);
        for offset in (0..7).rev() {
            let date = today - Duration::days(offset);
            let key = day_key(date);
            let day = self.days.get(&key);
            out.push(WeekDay {
                key,
                label: date.format("%a").to_string(),
                breaks: day.map(|d| d.breaks).unwrap_or(0),
                seconds: day.map(|d| d.seconds).unwrap_or(0),
                today: offset == 0,
            });
        }
        out
    }

    pub fn summary(&self, now: DateTime<Local>) -> Summary {
        let today = now.date_naive();
        let day = self.days.get(&day_key(today)).cloned().unwrap_or_default();
        let mut categories = BTreeMap::new();
        for day in self.days.values() {
            for (category, count) in &day.categories {
                *categories.entry(category.clone()).or_insert(0) += count;
            }
        }
        Summary {
            today_breaks: day.breaks,
            today_seconds: day.seconds,
            today_categories: day.categories.clone(),
            streak: self.compute_streak(today),
            total_breaks: self.totals.breaks,
            total_seconds: self.totals.seconds,
            categories,
            week: self.last_seven(today),
            recent: self.recent.iter().take(8).cloned().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(day: u32, hour: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 10, day, hour, 0, 0)
            .single()
            .unwrap()
    }

    fn stats_with(entries: &[(u32, &str, u32)]) -> StatsFile {
        let mut stats = StatsFile::default();
        for (day, category, seconds) in entries {
            stats.record("id", "Name", category, *seconds, at(*day, 12));
        }
        stats
    }

    #[test]
    fn recording_updates_day_totals_and_recent() {
        let mut stats = StatsFile::default();
        stats.record("walk", "Walkaround", "movement", 120, at(20, 9));
        stats.record("sip", "Sip of water", "hydration", 30, at(20, 10));

        let day = stats.days.get("2026-10-20").unwrap();
        assert_eq!(day.breaks, 2);
        assert_eq!(day.seconds, 150);
        assert_eq!(day.categories["movement"], 1);
        assert_eq!(day.categories["hydration"], 1);
        assert_eq!(stats.totals.breaks, 2);
        assert_eq!(stats.totals.seconds, 150);
        assert_eq!(stats.recent.len(), 2);
        assert_eq!(stats.recent[0].id, "sip");
        assert!(stats.first_break_at.is_some());
    }

    #[test]
    fn streak_counts_back_and_tolerates_an_empty_today() {
        let stats = stats_with(&[(19, "eyes", 20), (18, "eyes", 20), (17, "eyes", 20)]);
        assert_eq!(stats.compute_streak(at(20, 9).date_naive()), 3);
        let stats = stats_with(&[(20, "eyes", 20), (19, "eyes", 20), (15, "eyes", 20)]);
        assert_eq!(stats.compute_streak(at(20, 9).date_naive()), 2);
    }

    #[test]
    fn week_ends_with_today() {
        let stats = stats_with(&[(19, "eyes", 20)]);
        let week = stats.last_seven(at(20, 9).date_naive());
        assert_eq!(week.len(), 7);
        assert!(week[6].today);
        assert_eq!(week[6].key, "2026-10-20");
        assert_eq!(week[5].breaks, 1);
    }

    #[test]
    fn summary_reports_today_and_category_totals() {
        let stats = stats_with(&[(20, "eyes", 20), (20, "movement", 60), (19, "eyes", 20)]);
        let summary = stats.summary(at(20, 15));
        assert_eq!(summary.today_breaks, 2);
        assert_eq!(summary.today_seconds, 80);
        assert_eq!(summary.streak, 2);
        assert_eq!(summary.total_breaks, 3);
        assert_eq!(summary.total_seconds, 100);
        assert_eq!(summary.categories["eyes"], 2);
        assert_eq!(summary.week[6].breaks, 2);
    }
}
