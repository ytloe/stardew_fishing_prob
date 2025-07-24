//! src/calculator.rs
//!
//! 包含程序最核心的计算逻辑。
//! 使用确定性的序贯模型来精确计算钓鱼概率。

use crate::models::{AppConfig, GameData, ProbabilityDetails, ResolvedItem, SpawnFishData};
use crate::utils;
use itertools::Itertools;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

/// 顶层函数，获取在特定区域所有可能钓到的、经过充分过滤的物品列表。
pub fn get_resolved_fish_list<'a>(
    config: &AppConfig,
    game_data: &'a GameData,
    fish_area_id: &Option<String>,
    is_inherited: bool,
) -> Vec<ResolvedItem<'a>> {
    let mut call_stack = HashSet::new();
    resolve_location_fish(
        &config.location_name,
        config,
        game_data,
        &mut call_stack,
        fish_area_id,
        is_inherited,
    )
}

/// 递归核心：这个函数现在会执行所有能做的静态过滤，包括 Fish.json 里的天气等。
fn resolve_location_fish<'a>(
    location_name: &str,
    config: &AppConfig,
    game_data: &'a GameData,
    call_stack: &mut HashSet<String>,
    fish_area_id: &Option<String>,
    is_inherited: bool,
) -> Vec<ResolvedItem<'a>> {
    if call_stack.contains(location_name) { return vec![]; }
    call_stack.insert(location_name.to_string());

    let location_data = match game_data.locations.get(location_name) {
        Some(data) => data,
        None => { call_stack.remove(location_name); return vec![]; }
    };
    
    let using_magic_bait = config.bait_item_id.as_deref() == Some("(O)908");

    let possible_fish: Vec<&'a SpawnFishData> = game_data.locations["Default"]
        .fish.iter()
        .chain(location_data.fish.iter())
        .collect();

    let mut resolved_items = Vec::new();
    for spawn_data in possible_fish {
        let is_from_default = spawn_data as *const _ as usize <= game_data.locations["Default"].fish.last().unwrap() as *const _ as usize;
        if is_inherited && is_from_default && !spawn_data.can_be_inherited { continue; }
        if &spawn_data.fish_area_id != fish_area_id { continue; }
        
        if config.fishing_level < spawn_data.min_fishing_level { continue; }
        if config.water_depth < spawn_data.min_distance_from_shore { continue; }
        if spawn_data.max_distance_from_shore != -1 && config.water_depth > spawn_data.max_distance_from_shore as u32 { continue; }
        if spawn_data.require_magic_bait && !using_magic_bait { continue; }
        if let Some(id) = &spawn_data.item_id {
            if spawn_data.catch_limit == 1 && config.fish_caught.contains_key(id) { continue; }
        }
        if !utils::check_condition(&spawn_data.condition, config) { continue; }

        if !using_magic_bait {
            if let Some(season) = &spawn_data.season {
                if !season.to_lowercase().split_whitespace().any(|s| s == config.season) { continue; }
            }
            if !spawn_data.ignore_fish_data_requirements {
                if let Some(id) = &spawn_data.item_id {
                     if let Some(fish_data) = game_data.fish.get(id) {
                        if !fish_data.seasons.is_empty() && !fish_data.seasons.iter().any(|s| s == &config.season || s == "both") { continue; }
                        if fish_data.weather != "both" && fish_data.weather != config.weather { continue; }
                    }
                }
            }
        }
        
        resolved_items.extend(resolve_spawn_entry(spawn_data, config, game_data, call_stack, fish_area_id));
    }

    call_stack.remove(location_name);
    resolved_items
}


/// 解析单个 SpawnFishData 条目，处理特殊指令。
fn resolve_spawn_entry<'a>(
    spawn_data: &'a SpawnFishData,
    config: &AppConfig,
    game_data: &'a GameData,
    call_stack: &mut HashSet<String>,
    fish_area_id: &Option<String>,
) -> Vec<ResolvedItem<'a>> {
    let item_id_source = if let Some(item_id) = &spawn_data.item_id {
        item_id
    } else if let Some(random_ids) = &spawn_data.random_item_id {
        return vec![ResolvedItem {
            display_id: random_ids.join("|"),
            source_data: spawn_data,
        }];
    } else {
        return vec![];
    };

    let item_id = item_id_source.as_str();

    match item_id {
        "SECRET_NOTE_OR_ITEM" => {
            let has_all_notes = config.conditions.get("PLAYER_HAS_ALL_SECRET_NOTES") == Some(&"true".to_string());
            if has_all_notes { vec![] } 
            else { vec![ResolvedItem { display_id: item_id.to_string(), source_data: spawn_data }] }
        }
        id if id.starts_with("LOCATION_FISH") => {
            if let Some(target_location) = utils::parse_location_query(id) {
                resolve_location_fish(target_location, config, game_data, call_stack, fish_area_id, true)
            } else { vec![] }
        }
        _ => vec![ResolvedItem { display_id: item_id.to_string(), source_data: spawn_data }],
    }
}


/// 在一个具体的时间段内，对已解析的物品列表进行最终的动态筛选。
pub fn filter_items_for_time_segment<'a>(
    time_segment: (u32, u32),
    items: &'a [ResolvedItem<'a>],
    config: &AppConfig,
    game_data: &GameData,
) -> Vec<&'a ResolvedItem<'a>> {
    let (start_time, end_time) = time_segment;
    let using_magic_bait = config.bait_item_id.as_deref() == Some("(O)908");

    items.iter().filter(|item| {
        if using_magic_bait || item.source_data.ignore_fish_data_requirements { return true; }
        let Some(fish_data) = game_data.fish.get(&item.display_id) else { return true; };
        fish_data.time_windows.iter().any(|&(fish_start, fish_end)| start_time < fish_end && end_time > fish_start)
    }).collect()
}

/// 顶层函数：统一处理所有鱼饵类型的最终概率计算
pub fn calculate_final_probabilities<'a>(
    items: &[&'a ResolvedItem<'a>],
    config: &AppConfig,
    game_data: &GameData,
) -> Vec<ProbabilityDetails> {
    if items.is_empty() { return vec![]; }

    // --- 1. 预计算所有物品的单次成功率 ---
    let success_rates: HashMap<usize, f64> = items.par_iter().map(|&item| {
        let (get_chance, bite_chance) = get_individual_success_rates(item, config, game_data);
        (item as *const ResolvedItem as usize, get_chance * bite_chance)
    }).collect();

    // --- 2. 计算单次完整遍历（一个 "pass"）的捕获概率 ---
    let mut single_pass_probs: HashMap<usize, f64> = HashMap::new();
    {
        let mut groups: HashMap<i32, Vec<&'a ResolvedItem<'a>>> = HashMap::new();
        for &item in items {
            groups.entry(item.source_data.precedence).or_default().push(item);
        }
        let mut sorted_precedences: Vec<i32> = groups.keys().cloned().collect();
        sorted_precedences.sort();

        let mut p_uncaught_so_far = 1.0;
        for precedence in sorted_precedences {
            let group = &groups[&precedence];
            let group_catch_probs = calculate_group_probabilities(group, &success_rates);
            let p_catch_none_in_group = 1.0 - group_catch_probs.values().sum::<f64>();
            for (ptr, prob) in group_catch_probs {
                *single_pass_probs.entry(ptr).or_insert(0.0) += p_uncaught_so_far * prob;
            }
            p_uncaught_so_far *= p_catch_none_in_group;
        }
    }

    // --- 3. 根据鱼饵类型，组合多次尝试的结果 ---
    let final_probabilities = if let Some(target_fish_id) = &config.bait_target_fish_id {
        // --- 特制鱼饵的逻辑 ---
        // 特制鱼饵本身就是一种“好鱼饵”，所以固定两次尝试
        let passes = 2; 
        
        // 安全地找到目标鱼的指针
        let target_ptr_opt = items.iter()
            .find(|item| item.display_id == *target_fish_id)
            .map(|item| *item as *const _ as usize);

        let p_catch_target_once = if let Some(ptr) = target_ptr_opt {
            single_pass_probs.get(&ptr).cloned().unwrap_or(0.0)
        } else { 0.0 };

        let p_final_target = 1.0 - (1.0 - p_catch_target_once).powi(passes);
        
        let mut final_probs_map = HashMap::new();
        if let Some(ptr) = target_ptr_opt {
             final_probs_map.insert(ptr, p_final_target);
        }
        
        let sum_prob_nontarget_once: f64 = single_pass_probs.iter()
            .filter(|(ptr, _)| Some(**ptr) != target_ptr_opt)
            .map(|(_, prob)| *prob)
            .sum();

        if sum_prob_nontarget_once > 0.0 {
            let p_remaining = 1.0 - p_final_target;
            for (item_ptr, prob_once) in &single_pass_probs {
                if Some(*item_ptr) != target_ptr_opt {
                    let p_final_nontarget = p_remaining * (prob_once / sum_prob_nontarget_once);
                    final_probs_map.insert(*item_ptr, p_final_nontarget);
                }
            }
        }
        final_probs_map

    } else {
        // --- 标准/好鱼饵的逻辑 ---
        let passes = if config.using_good_bait { 2 } else { 1 };
        let mut final_probs_map = HashMap::new();
        let mut p_uncaught_overall = 1.0;

        for _ in 0..passes {
            for (ptr, prob_this_pass) in &single_pass_probs {
                *final_probs_map.entry(*ptr).or_insert(0.0) += p_uncaught_overall * prob_this_pass;
            }
            let p_caught_this_pass: f64 = single_pass_probs.values().sum();
            p_uncaught_overall *= 1.0 - p_caught_this_pass;
        }
        final_probs_map
    };

    // --- 4. 聚合结果并返回 ---
    let mut results: Vec<ProbabilityDetails> = items.iter().map(|&item| {
        let item_ptr = item as *const ResolvedItem as usize;
        let (get_chance, bite_chance) = get_individual_success_rates(item, config, game_data);
        let final_prob = final_probabilities.get(&item_ptr).cloned().unwrap_or(0.0);
        
        ProbabilityDetails {
            display_id: item.display_id.clone(),
            name: get_resolved_item_name(item, game_data),
            precedence: item.source_data.precedence,
            get_chance_prob: get_chance,
            bite_chance_prob: bite_chance,
            final_prob,
        }
    }).collect();

    results.sort_by(|a, b| b.final_prob.partial_cmp(&a.final_prob).unwrap_or(std::cmp::Ordering::Equal));
    results
}

/// 确定性计算一个同优先级组内，钓到每条鱼的概率
fn calculate_group_probabilities<'a>(
    group: &[&'a ResolvedItem<'a>],
    success_rates: &HashMap<usize, f64>,
) -> HashMap<usize, f64> {
    if group.is_empty() { return HashMap::new(); }

    let num_items = group.len();
    let mut total_catch_probs: HashMap<usize, f64> = HashMap::new();

    if num_items > 8 { // Performance safeguard
        let total_success_rate: f64 = group.iter().map(|item| {
            let item_ptr = &**item as *const ResolvedItem as usize;
            success_rates.get(&item_ptr).cloned().unwrap_or(0.0)
        }).sum();

        if total_success_rate > 0.0 {
            let normalization_factor = (1.0 - (1.0 - total_success_rate).max(0.0).min(1.0)) / total_success_rate;
            for &item in group {
                let item_ptr = item as *const ResolvedItem as usize;
                let p_success = success_rates.get(&item_ptr).cloned().unwrap_or(0.0);
                total_catch_probs.insert(item_ptr, p_success * normalization_factor);
            }
        }
        return total_catch_probs;
    }

    for permutation in group.iter().permutations(num_items) {
        let mut p_uncaught_in_perm = 1.0;
        for &&item in &permutation {
            let item_ptr = item as *const ResolvedItem as usize;
            let p_success = success_rates.get(&item_ptr).cloned().unwrap_or(0.0);
            
            let p_catch_this = p_uncaught_in_perm * p_success;
            *total_catch_probs.entry(item_ptr).or_insert(0.0) += p_catch_this;
            
            p_uncaught_in_perm *= 1.0 - p_success;
        }
    }

    let num_permutations = (1..=num_items).map(|i| i as f64).product::<f64>();

    if num_permutations > 0.0 {
        for prob in total_catch_probs.values_mut() { *prob /= num_permutations; }
    }

    total_catch_probs
}

/// 计算单个物品的“存活概率”和“咬钩概率”
fn get_individual_success_rates(item: &ResolvedItem, config: &AppConfig, game_data: &GameData) -> (f64, f64) {
    let is_targeted = config.bait_target_fish_id.as_deref() == Some(&item.display_id);
    
    let mut get_chance_prob = item.source_data.chance;
    if config.has_curiosity_lure && item.source_data.curiosity_lure_buff > 0.0 {
        get_chance_prob += item.source_data.curiosity_lure_buff;
    }
    if item.source_data.apply_daily_luck {
        get_chance_prob += config.daily_luck;
    }
    if is_targeted {
        get_chance_prob = get_chance_prob * item.source_data.specific_bait_multiplier + item.source_data.specific_bait_buff;
    }
    get_chance_prob += item.source_data.chance_boost_per_luck_level * config.luck_level as f64;
    
    let mut bite_chance_prob = 1.0;
    if !item.source_data.ignore_fish_data_requirements {
        if let Some(fish_data) = game_data.fish.get(&item.display_id) {
            let mut passes = true;
            if config.is_training_rod {
                if let Some(false) = item.source_data.can_use_training_rod { passes = false; }
                else if item.source_data.can_use_training_rod.is_none() && fish_data.difficulty >= 50 { passes = false; }
            }
            if config.is_tutorial_catch && !fish_data.is_tutorial_fish { passes = false; }
            if fish_data.min_fishing_level > config.fishing_level { passes = false; }

            if !passes {
                bite_chance_prob = 0.0;
            } else {
                let mut chance = fish_data.base_chance;
                let drop_off_amount = fish_data.depth_multiplier * chance;
                chance -= (fish_data.max_depth as f64 - config.water_depth as f64).max(0.0) * drop_off_amount;
                chance += config.fishing_level as f64 / 50.0;
                
                if config.is_training_rod { chance *= 1.1; }
                chance = chance.min(0.9);
                
                if config.has_curiosity_lure && chance < 0.25 {
                    if item.source_data.curiosity_lure_buff > -1.0 {
                        chance += item.source_data.curiosity_lure_buff;
                    } else {
                        let max = 0.25; let min = 0.08;
                        chance = (max - min) / max * chance + (max - min) / 2.0;
                    }
                }
                
                if is_targeted { chance *= 1.66; }
                if item.source_data.apply_daily_luck { chance += config.daily_luck; }
                
                bite_chance_prob = chance;
            }
        }
    }
    
    (get_chance_prob.clamp(0.0, 1.0), bite_chance_prob.clamp(0.0, 1.0))
}

/// 获取物品的最终显示/聚合名称
fn get_resolved_item_name(item: &ResolvedItem, game_data: &GameData) -> String {
    if item.display_id.contains('|') { return "Trash".to_string(); }
    game_data.fish.get(&item.display_id)
        .map(|data| data.name.clone())
        .unwrap_or_else(|| item.display_id.clone())
}