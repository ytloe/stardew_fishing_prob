//! src/utils.rs
//!
//! 存放可复用的、无状态的工具函数。
//! 遵循模块化原则，保持其他模块的逻辑清晰。

use crate::models::{AppConfig, GameData, ResolvedItem};
use std::collections::BTreeSet;

/// 解析LOCATION_FISH查询，如果成功则返回目标地点名称。
pub fn parse_location_query(item_id: &str) -> Option<&str> {
    let parts: Vec<&str> = item_id.split_whitespace().collect();
    if parts.get(0) == Some(&"LOCATION_FISH") {
        return parts.get(1).cloned();
    }
    None
}

/// 根据已解析的物品列表，计算出所有需要分析的时间段。
pub fn calculate_time_segments(
    resolved_items: &[ResolvedItem],
    game_data: &GameData,
) -> Vec<(u32, u32)> {
    let mut time_points = BTreeSet::new();
    time_points.insert(600);
    time_points.insert(2600);

    for item in resolved_items {
        if let Some(fish_data) = game_data.fish.get(&item.display_id) {
            for &(start, end) in &fish_data.time_windows {
                time_points.insert(start);
                time_points.insert(end);
            }
        }
    }

    let sorted_points: Vec<u32> = time_points.into_iter().collect();
    sorted_points
        .windows(2)
        .map(|window| (window[0], window[1]))
        .collect()
}


/// 通用的 Condition 检查器。
pub fn check_condition(condition_str: &Option<String>, config: &AppConfig) -> bool {
    let Some(conditions) = condition_str else { return true; };
    for query in conditions.split(',') {
        if !evaluate_query(query.trim(), config) {
            return false;
        }
    }
    true
}

fn evaluate_query(query: &str, config: &AppConfig) -> bool {
    let (is_negated, trimmed_query) = if let Some(q) = query.strip_prefix('!') {
        (true, q)
    } else {
        (false, query)
    };

    let parts: Vec<&str> = trimmed_query.split_whitespace().collect();
    let Some(&key) = parts.get(0) else { return true; };
    let args = &parts[1..];

    // --- 查询调度中心 ---
    let result = match key {
        "LOCATION_SEASON" => {
            if args.get(0) == Some(&"Here") {
                let valid_seasons = &args[1..];
                // --- 修正：使用 .any() 来检查是否包含当前季节 ---
                valid_seasons.iter().any(|&s| s == config.season.as_str())
            } else { false }
        },
        "PLAYER_SPECIAL_ORDER_RULE_ACTIVE" => {
            if args.len() == 2 && args[0] == "Current" {
                let required_order_id = args[1];
                // --- 修正：精确匹配 config 中设置的规则 ID ---
                 config.conditions.get("PLAYER_SPECIAL_ORDER_RULE_ACTIVE Current")
                    .map_or(false, |active_rule| active_rule == required_order_id)
            } else { false }
        },
        _ => {
            // 默认行为：检查条件是否存在且为 "true"
            config.conditions.get(trimmed_query).map_or(false, |v| v == "true")
        }
    };

    if is_negated { !result } else { result }
}


/// 将字符串截断到指定的最大宽度，如果发生截断则添加"..."
pub fn truncate_string(s: &str, max_width: usize) -> String {
    if s.chars().count() <= max_width {
        return s.to_string();
    }
    
    if max_width < 3 {
        return s.chars().take(max_width).collect();
    }
    
    format!("{}...", s.chars().take(max_width - 3).collect::<String>())
}