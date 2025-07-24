//! src/models.rs
//!
//! 定义了程序中所有核心的数据结构。

use serde::Deserialize;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct FishAreaData {
    pub display_name: Option<String>,
    pub position: Option<Rect>,
    #[serde(default)]
    pub crab_pot_fish_types: Vec<String>,
    #[serde(default)]
    pub crab_pot_junk_chance: f64,
}


/// 代表从 Locations.json 中 'Fish' 数组里的一个条目。
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct SpawnFishData {
    pub item_id: Option<String>,
    #[serde(default)]
    pub random_item_id: Option<Vec<String>>,
    pub precedence: i32,
    pub chance: f64,
    #[serde(default)]
    pub ignore_fish_data_requirements: bool,
    #[serde(default = "default_bait_multiplier")]
    pub specific_bait_multiplier: f64,
    #[serde(default)]
    pub specific_bait_buff: f64,
    pub condition: Option<String>,
    pub season: Option<String>,
    #[serde(default)]
    pub min_distance_from_shore: u32,
    #[serde(default = "default_max_dist")]
    pub max_distance_from_shore: i32,
    #[serde(default = "default_curiosity_lure_buff")]
    pub curiosity_lure_buff: f64,
    #[serde(default)]
    pub apply_daily_luck: bool,
    #[serde(default)]
    pub chance_boost_per_luck_level: f64,
    pub fish_area_id: Option<String>,
    pub bobber_position: Option<Rect>,
    pub player_position: Option<Rect>,
    #[serde(default)]
    pub min_fishing_level: u32,
    #[serde(default = "default_catch_limit")]
    pub catch_limit: i32,
    pub can_use_training_rod: Option<bool>,
    #[serde(default)]
    pub is_boss_fish: bool,
    pub set_flag_on_catch: Option<String>,
    #[serde(default)]
    pub require_magic_bait: bool,
    #[serde(default = "default_can_be_inherited")]
    pub can_be_inherited: bool,
    #[serde(default)]
    pub use_fish_caught_seeded_random: bool,
}
fn default_bait_multiplier() -> f64 { 1.66 }
fn default_max_dist() -> i32 { -1 }
fn default_curiosity_lure_buff() -> f64 { -1.0 }
fn default_catch_limit() -> i32 { -1 }
fn default_can_be_inherited() -> bool { true }

/// 代表从 Locations.json 中的一个地点条目，例如 "Town" 或 "Forest"。
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct LocationData {
    pub fish: Vec<SpawnFishData>,
    #[serde(default)]
    pub fish_areas: HashMap<String, FishAreaData>,
}

/// 代表解析后的 Fish.json 条目。
#[derive(Debug, Clone)]
pub struct ParsedFishData {
    pub name: String,
    pub difficulty: u32,
    pub time_windows: Vec<(u32, u32)>,
    pub seasons: Vec<String>,
    pub weather: String,
    pub min_fishing_level: u32,
    pub max_depth: u32,
    pub base_chance: f64,
    pub depth_multiplier: f64,
    pub is_tutorial_fish: bool,
}

/// 一个聚合所有游戏数据的容器，便于在函数间传递。
pub struct GameData {
    pub locations: HashMap<String, LocationData>,
    pub fish: HashMap<String, ParsedFishData>,
    pub fish_name_to_id: HashMap<String, String>,
}

/// 一个被完全解析后的可捕获物品。
#[derive(Debug, Clone)]
pub struct ResolvedItem<'a> {
    pub display_id: String,
    pub source_data: &'a SpawnFishData,
}

impl<'a> PartialEq for ResolvedItem<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.display_id == other.display_id 
        && std::ptr::eq(self.source_data, other.source_data)
    }
}
impl<'a> Eq for ResolvedItem<'a> {}
impl<'a> Hash for ResolvedItem<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.display_id.hash(state);
        (self.source_data as *const SpawnFishData).hash(state);
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct StringMap {
    pub seasons: HashMap<String, String>,
    pub weather: HashMap<String, String>,
    pub rod_internal_ids: HashMap<String, String>,
    pub bait_internal_ids: HashMap<String, String>,
    pub tackle_internal_ids: HashMap<String, String>,
    #[serde(default)]
    pub fish_names: HashMap<String, String>,
}

fn default_water_depth() -> u32 { 4 }

/// 代表从 config.json 加载的原始用户输入。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UserConfigRaw {
    #[serde(default)]
    pub is_tutorial_catch: bool,
    pub location_name: String,
    pub rod_type: String,
    pub bait_type: String,
    #[serde(default)]
    pub tackles: Vec<String>,
    pub weather: String,
    pub season: String,
    pub fishing_level: u32,
    pub luck_level: u32,
    pub daily_luck: f64,
    #[serde(default = "default_water_depth")]
    pub water_depth: u32,
    #[serde(default)]
    pub conditions: HashMap<String, String>,
    #[serde(default)]
    pub fish_caught: Vec<(String, u32)>,
}

/// 解析后，供程序内部所有计算函数使用的最终配置。
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub is_tutorial_catch: bool,
    pub is_training_rod: bool,
    pub using_good_bait: bool,
    pub bait_item_id: Option<String>,
    pub bait_target_fish_id: Option<String>,
    pub has_curiosity_lure: bool,
    pub location_name: String,
    pub season: String,
    pub weather: String,
    pub water_depth: u32,
    pub fishing_level: u32,
    pub luck_level: u32,
    pub daily_luck: f64,
    pub conditions: HashMap<String, String>,
    pub fish_caught: HashMap<String, u32>,
}

/// 用于在main函数中传递和打印最终详细概率信息的结构体。
#[derive(Debug, Clone)]
pub struct ProbabilityDetails {
    pub display_id: String,
    pub name: String,
    pub precedence: i32,
    pub get_chance_prob: f64,
    pub bite_chance_prob: f64,
    pub final_prob: f64,
}