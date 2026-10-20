use std::collections::HashMap;

use rand::seq::SliceRandom;

use crate::breaks::BreakItem;
use crate::config::Categories;

pub struct Rotation {
    items: Vec<BreakItem>,
    category_bag: Vec<String>,
    break_bags: HashMap<String, Vec<String>>,
    last_category: Option<String>,
}

impl Rotation {
    pub fn new(items: Vec<BreakItem>) -> Self {
        Self {
            items,
            category_bag: Vec::new(),
            break_bags: HashMap::new(),
            last_category: None,
        }
    }

    pub fn has_any(&self, enabled: &Categories) -> bool {
        self.items
            .iter()
            .any(|item| enabled.enabled(&item.category))
    }

    pub fn enabled_categories(&self, enabled: &Categories) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for item in &self.items {
            if enabled.enabled(&item.category) && !out.contains(&item.category) {
                out.push(item.category.clone());
            }
        }
        out
    }

    pub fn next(&mut self, enabled: &Categories) -> Option<BreakItem> {
        let categories = self.enabled_categories(enabled);
        if categories.is_empty() {
            return None;
        }

        self.category_bag
            .retain(|category| categories.contains(category));
        if self.category_bag.is_empty() {
            let mut bag = categories.clone();
            bag.shuffle(&mut rand::thread_rng());
            if bag.len() > 1 && Some(&bag[0]) == self.last_category.as_ref() {
                bag.swap(0, 1);
            }
            self.category_bag = bag;
        }

        let category = self.category_bag.remove(0);
        let pool: Vec<&BreakItem> = self
            .items
            .iter()
            .filter(|item| item.category == category && enabled.enabled(&item.category))
            .collect();

        let mut bag: Vec<String> = self
            .break_bags
            .get(&category)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|id| pool.iter().any(|item| &item.id == id))
            .collect();
        if bag.is_empty() {
            bag = pool.iter().map(|item| item.id.clone()).collect();
            bag.shuffle(&mut rand::thread_rng());
        }

        let id = bag.remove(0);
        self.break_bags.insert(category.clone(), bag);
        self.last_category = Some(category);
        pool.into_iter().find(|item| item.id == id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, category: &str) -> BreakItem {
        BreakItem {
            id: id.into(),
            category: category.into(),
            name: id.into(),
            instruction: String::new(),
            spec: String::new(),
            duration_seconds: 30,
            figure: String::new(),
            steps: Vec::new(),
            tip: String::new(),
        }
    }

    fn all_on() -> Categories {
        Categories {
            eyes: true,
            hydration: true,
            breathing: true,
            movement: true,
        }
    }

    #[test]
    fn categories_rotate_evenly() {
        let items = vec![
            item("e1", "eyes"),
            item("e2", "eyes"),
            item("h1", "hydration"),
            item("b1", "breathing"),
            item("m1", "movement"),
            item("m2", "movement"),
            item("m3", "movement"),
        ];
        let mut rotation = Rotation::new(items);
        let mut counts = HashMap::new();
        for _ in 0..8 {
            let chosen = rotation.next(&all_on()).unwrap();
            *counts.entry(chosen.category).or_insert(0) += 1;
        }
        assert_eq!(counts["eyes"], 2);
        assert_eq!(counts["hydration"], 2);
        assert_eq!(counts["breathing"], 2);
        assert_eq!(counts["movement"], 2);
    }

    #[test]
    fn breaks_cycle_before_repeating_within_a_category() {
        let items = vec![
            item("m1", "movement"),
            item("m2", "movement"),
            item("m3", "movement"),
        ];
        let mut rotation = Rotation::new(items);
        let enabled = Categories {
            eyes: false,
            hydration: false,
            breathing: false,
            movement: true,
        };
        let mut seen = std::collections::HashSet::new();
        for _ in 0..3 {
            seen.insert(rotation.next(&enabled).unwrap().id);
        }
        assert_eq!(seen.len(), 3);
    }

    #[test]
    fn disabled_categories_are_never_selected() {
        let items = vec![
            item("e1", "eyes"),
            item("h1", "hydration"),
            item("m1", "movement"),
        ];
        let mut rotation = Rotation::new(items);
        let enabled = Categories {
            eyes: true,
            hydration: false,
            breathing: false,
            movement: true,
        };
        for _ in 0..20 {
            let chosen = rotation.next(&enabled).unwrap();
            assert!(chosen.category == "eyes" || chosen.category == "movement");
        }
    }

    #[test]
    fn nothing_enabled_returns_none() {
        let mut rotation = Rotation::new(vec![item("e1", "eyes")]);
        let off = Categories {
            eyes: false,
            hydration: false,
            breathing: false,
            movement: false,
        };
        assert!(!rotation.has_any(&off));
        assert!(rotation.next(&off).is_none());
    }
}
