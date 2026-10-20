use std::collections::BTreeMap;

use rust_embed::Embed;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakItem {
    pub id: String,
    pub category: String,
    pub name: String,
    pub instruction: String,
    pub spec: String,
    pub duration_seconds: u32,
    pub figure: String,
    pub steps: Vec<String>,
    pub tip: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CategoryMeta {
    pub name: String,
    pub blurb: String,
    pub color: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Breaks {
    pub categories: BTreeMap<String, CategoryMeta>,
    pub breaks: Vec<BreakItem>,
}

#[derive(Embed)]
#[folder = "../assets/figures"]
struct Figures;

pub fn load() -> Breaks {
    serde_json::from_str(include_str!("../../data/breaks.json"))
        .expect("breaks.json should be valid")
}

pub fn figure_svg(path: &str) -> String {
    let key = path.strip_prefix("figures/").unwrap_or(path);
    match Figures::get(key) {
        Some(file) => String::from_utf8_lossy(&file.data).into_owned(),
        None => {
            eprintln!("missing figure: {path}");
            String::new()
        }
    }
}
