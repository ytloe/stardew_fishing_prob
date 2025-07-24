//! src/config.rs
//!
//! 负责加载和解析所有配置文件。
//! 它的主要职责是将用户提供的、人类可读的配置 (UserConfigRaw)，
//! 结合游戏数据 (GameData)，转换为程序内部使用的、
//! 经过精确处理的配置 (AppConfig)。

use crate::models::{
    AppConfig, GameData, LocationData, ParsedFishData, StringMap, UserConfigRaw,
};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub fn load_and_build_config() -> Result<(AppConfig, GameData), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let config_path = manifest_dir.join("config.json");
    let locations_path = manifest_dir.join("data/Locations.json");
    let fish_path = manifest_dir.join("data/Fish.json");
    let string_map_path = manifest_dir.join("data/StringMap.json");

    let raw_config: UserConfigRaw = serde_json::from_str(&fs::read_to_string(config_path)?)?;
    let locations: HashMap<String, LocationData> = serde_json::from_str(&fs::read_to_string(locations_path)?)?;
    let raw_fish_data: HashMap<String, String> = serde_json::from_str(&fs::read_to_string(fish_path)?)?;
    let string_map: StringMap = serde_json::from_str(&fs::read_to_string(string_map_path)?)?;
    
    let (fish, fish_name_to_id) = parse_fish_data(raw_fish_data)?;
    
    let game_data = GameData {
        locations,
        fish,
        fish_name_to_id,
    };

    let app_config = build_app_config(&raw_config, &game_data, &string_map)
        .map_err(|e| Box::<dyn std::error::Error>::from(e))?;

    Ok((app_config, game_data))
}

/// 解析 Fish.json 的原始字符串数据，将其转换为结构化的 ParsedFishData。
fn parse_fish_data(
    raw_data: HashMap<String, String>,
) -> Result<(HashMap<String, ParsedFishData>, HashMap<String, String>), String> {
    let mut fish = HashMap::new();
    let mut fish_name_to_id = HashMap::new();

    for (id, value) in raw_data {
        let parts: Vec<&str> = value.split('/').collect();
        // 增加对 trap 鱼的过滤
        if parts.get(1) == Some(&"trap") || parts.len() < 13 { continue; }

        let name = parts[0].to_string();

        // 解析时间窗口
        let time_str_parts: Vec<&str> = parts[5].split_whitespace().collect();
        let mut time_windows = Vec::new();
        for chunk in time_str_parts.chunks(2) {
            if chunk.len() == 2 {
                if let (Ok(start), Ok(end)) = (chunk[0].parse::<u32>(), chunk[1].parse::<u32>()) {
                    time_windows.push((start, end));
                }
            }
        }

        let seasons: Vec<String> = parts[6].split_whitespace().map(|s| s.to_string()).collect();

        let parsed = ParsedFishData {
            name: name.clone(),
            difficulty: parts[1].parse().unwrap_or(0),
            time_windows,
            seasons,
            weather: parts[7].to_string(),
            max_depth: parts[9].parse().unwrap_or(4),
            min_fishing_level: parts[12].parse().unwrap_or(0),
            base_chance: parts[10].parse().unwrap_or(0.0),
            depth_multiplier: parts[11].parse().unwrap_or(0.0),
            is_tutorial_fish: parts.get(13).map_or(false, |&s| s.parse().unwrap_or(false)),
        };

        let item_id = format!("(O){}", id);
        fish.insert(item_id.clone(), parsed);
        fish_name_to_id.insert(name, item_id);
    }

    Ok((fish, fish_name_to_id))
}


fn build_app_config(
    raw_config: &UserConfigRaw,
    game_data: &GameData,
    string_map: &StringMap,
) -> Result<AppConfig, String> {
    let is_training_rod = string_map.rod_internal_ids.get(&raw_config.rod_type).is_some();

    let (bait_item_id, bait_target_fish_id, using_good_bait) =
        match raw_config.bait_type.as_str() {
            "None" | "" => (None, None, false),
            bait_name if string_map.bait_internal_ids.contains_key(bait_name) => {
                let item_id = string_map.bait_internal_ids[bait_name].clone();
                let good_bait = item_id != "(O)685";
                (Some(item_id), None, good_bait)
            }
            specific_bait_name => {
                let mut target_id = None;
                if let Some(fish_name_cn) = specific_bait_name.strip_suffix("鱼饵") {
                    if let Some(fish_name_en) = string_map.fish_names.get(fish_name_cn) {
                        if let Some(fish_id) = game_data.fish_name_to_id.get(fish_name_en) {
                            target_id = Some(fish_id.clone());
                        }
                    }
                } else if let Some(fish_name_en) = specific_bait_name.strip_suffix(" Bait") {
                    if let Some(fish_id) = game_data.fish_name_to_id.get(fish_name_en) {
                        target_id = Some(fish_id.clone());
                    }
                }
                if let Some(id) = target_id {
                    (Some("(O)SpecificBait".to_string()), Some(id), true)
                } else {
                    return Err(format!("无法识别的特制鱼饵: {}", specific_bait_name));
                }
            }
        };

    let has_curiosity_lure = raw_config.tackles.iter()
        .any(|tackle_name| string_map.tackle_internal_ids.get(tackle_name).is_some());

    let season = string_map.seasons.get(&raw_config.season)
        .ok_or_else(|| format!("Invalid season: {}", raw_config.season))?.clone();
    let weather = string_map.weather.get(&raw_config.weather)
        .ok_or_else(|| format!("Invalid weather: {}", raw_config.weather))?.clone();
        
    Ok(AppConfig {
        is_tutorial_catch: raw_config.is_tutorial_catch,
        is_training_rod,
        using_good_bait,
        bait_item_id,
        bait_target_fish_id,
        has_curiosity_lure,
        location_name: raw_config.location_name.clone(),
        season,
        weather,
        water_depth: raw_config.water_depth,
        fishing_level: raw_config.fishing_level,
        luck_level: raw_config.luck_level,
        daily_luck: raw_config.daily_luck,
        conditions: raw_config.conditions.clone(),
        fish_caught: raw_config.fish_caught.clone().into_iter().collect(),
    })
}