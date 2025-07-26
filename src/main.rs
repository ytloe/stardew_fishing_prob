//! src/main.rs

use std::collections::{HashMap, HashSet};

mod config;
mod models;
mod calculator;
mod utils;

fn main() {
    // 1. 加载所有配置和游戏数据
    let (app_config, game_data) = match config::load_and_build_config() {
        Ok((config, data)) => (config, data),
        Err(e) => {
            eprintln!("\n错误：加载配置失败。\n原因: {}", e);
            return;
        }
    };
    
    // 2. 获取并准备遍历所有 FishAreas
    let location_data = game_data.locations.get(&app_config.location_name)
        .expect("Location data should exist");
    
    let mut fish_area_ids: Vec<Option<String>> = location_data.fish_areas.keys().cloned().map(Some).collect();
    let has_default_area_fish = location_data.fish.iter().any(|f| f.fish_area_id.is_none()) 
                            || game_data.locations["Default"].fish.iter().any(|f| f.fish_area_id.is_none());
    if has_default_area_fish {
        if !fish_area_ids.contains(&None) { fish_area_ids.push(None); }
    }

    // 3. 主逻辑
    for area_id in fish_area_ids {
        let base_items = calculator::get_resolved_fish_list(&app_config, &game_data, &area_id, false);
        let time_segments = utils::calculate_time_segments(&base_items, &game_data);
        
        for segment in time_segments {
            let segment_items = calculator::filter_items_for_time_segment(segment, &base_items, &app_config, &game_data);
            if segment_items.is_empty() { continue; }

            let area_name = if let Some(id) = &area_id { id.as_str() } else { "Default" };
            println!("\nLocation: {} ({})", app_config.location_name, area_name);
            println!("Time: {:04}-{:04}", segment.0, segment.1);

            // --- 核心逻辑分支：根据是否为魔法鱼饵选择不同的输出模式 ---
            if app_config.bait_item_id.as_deref() == Some("(O)908") {
                // --- 魔法鱼饵的简单列表输出 ---
                let detailed_probabilities = calculator::calculate_final_probabilities(&segment_items, &app_config, &game_data);
                
                // 1. 将结果转换为 Map 以便查找
                let prob_map: HashMap<String, f64> = detailed_probabilities.into_iter()
                    .map(|p| (p.display_id, p.final_prob))
                    .collect();

                // 2. 按优先级排序原始物品列表以确定行序
                let mut row_items = segment_items.clone();
                row_items.sort_by_key(|item| item.source_data.precedence);

                // 3. 聚合垃圾项
                const TRASH_GROUP_SOURCE_ID: &str = "(O)167|(O)168|(O)169|(O)170|(O)171|(O)172";
                let mut aggregated_rows: Vec<(String, String, i32, f64)> = Vec::new(); // (ID, Name, Prio, Prob)
                let mut trash_aggregator: Option<(String, String, i32, f64)> = None;
                let mut handled_source_data: HashSet<*const models::SpawnFishData> = HashSet::new();

                for item in &row_items {
                    let source_ptr = item.source_data as *const _;
                    if handled_source_data.contains(&source_ptr) { continue; }

                    if item.source_data.id.as_deref() == Some(TRASH_GROUP_SOURCE_ID) {
                        if trash_aggregator.is_none() {
                            trash_aggregator = Some(("Trash Group".to_string(), "Trash".to_string(), item.source_data.precedence, 0.0));
                        }
                        // 找到所有属于这个源的兄弟项并聚合它们的概率
                        for sibling in row_items.iter().filter(|i| i.source_data as *const _ == source_ptr) {
                            if let Some(prob) = prob_map.get(&sibling.display_id) {
                                if let Some((_, _, _, agg_prob)) = &mut trash_aggregator {
                                    *agg_prob += prob;
                                }
                            }
                        }
                    } else {
                        let prob = prob_map.get(&item.display_id).cloned().unwrap_or(0.0);
                        let name = calculator::get_resolved_item_name(item, &game_data);
                        aggregated_rows.push((item.display_id.clone(), name, item.source_data.precedence, prob));
                    }
                    handled_source_data.insert(source_ptr);
                }

                if let Some(agg_trash) = trash_aggregator {
                    aggregated_rows.push(agg_trash);
                }
                aggregated_rows.sort_by_key(|(_, _, prio, _)| *prio);

                // 4. 打印简化的表格
                println!("{:<25} | {:<25} | {:<5} | {}", "ID", "Name", "Prio", "Final Prob");
                println!("{:-<25}-+-{:-<25}-+-{:-<5}-+-{:-<15}", "", "", "", "");

                for (id, name, prio, prob) in &aggregated_rows {
                    println!(
                        "{:<25} | {:<25} | {:<5} | {:>12.2}%",
                        utils::truncate_string(id, 23),
                        utils::truncate_string(name, 23),
                        prio,
                        prob * 100.0
                    );
                }
            } else {
                // --- 其他鱼饵的多列对比表格输出 ---
                let mut row_items = segment_items.clone();
                row_items.sort_by_key(|item| item.source_data.precedence);
                
                let mut scenarios = Vec::new();
                
                let mut standard_config = app_config.clone();
                standard_config.bait_item_id = None;
                standard_config.bait_target_fish_id = None;
                standard_config.is_training_rod = false;
                scenarios.push(("Standard".to_string(), standard_config.clone()));

                let mut training_rod_config = standard_config.clone();
                training_rod_config.is_training_rod = true;
                scenarios.push(("TrainingRod".to_string(), training_rod_config));

                let mut bait_fish_scenarios = Vec::new();
                let mut handled_baits = HashSet::new();
                for &item in &segment_items {
                    if game_data.fish.contains_key(&item.display_id) && handled_baits.insert(item.display_id.clone()) {
                        let mut bait_config = standard_config.clone();
                        bait_config.bait_item_id = Some("(O)SpecificBait".to_string());
                        bait_config.bait_target_fish_id = Some(item.display_id.clone());
                        bait_config.using_good_bait = true;
                        let fish_name_en = &game_data.fish[&item.display_id].name;
                        bait_fish_scenarios.push((fish_name_en.clone(), bait_config));
                    }
                }
                bait_fish_scenarios.sort_by_key(|(_name, cfg)| {
                    segment_items.iter().find(|item| &item.display_id == cfg.bait_target_fish_id.as_ref().unwrap())
                    .map_or(i32::MAX, |item| item.source_data.precedence)
                });
                scenarios.extend(bait_fish_scenarios);

                let mut results_map: HashMap<String, Vec<f64>> = HashMap::new();
                for (_, scenario_config) in &scenarios {
                    let scenario_probs = calculator::calculate_final_probabilities(&segment_items, scenario_config, &game_data);
                    let scenario_probs_map: HashMap<String, f64> = scenario_probs.into_iter()
                        .map(|p| (p.display_id, p.final_prob)).collect();
                    for item in &row_items {
                        let prob = scenario_probs_map.get(&item.display_id).cloned().unwrap_or(0.0);
                        results_map.entry(item.display_id.clone()).or_default().push(prob);
                    }
                }
                
                const TRASH_GROUP_SOURCE_ID: &str = "(O)167|(O)168|(O)169|(O)170|(O)171|(O)172";
                let mut aggregated_rows: Vec<(String, i32, Vec<f64>)> = Vec::new();
                let mut trash_aggregator: Option<(String, i32, Vec<f64>)> = None;
                let mut handled_source_data: HashSet<*const models::SpawnFishData> = HashSet::new();

                for item in &row_items {
                    let source_ptr = item.source_data as *const _;
                    if handled_source_data.contains(&source_ptr) { continue; }

                    let item_name = calculator::get_resolved_item_name(item, &game_data);
                    
                    if item.source_data.id.as_deref() == Some(TRASH_GROUP_SOURCE_ID) {
                        if trash_aggregator.is_none() {
                            trash_aggregator = Some(("Trash Group".to_string(), item.source_data.precedence, vec![0.0; scenarios.len()]));
                        }
                        for sibling in row_items.iter().filter(|i| i.source_data as *const _ == source_ptr) {
                            if let Some(probs) = results_map.get(&sibling.display_id) {
                                if let Some((_, _, agg_probs)) = &mut trash_aggregator {
                                    for (i, prob) in probs.iter().enumerate() {
                                        agg_probs[i] += prob;
                                    }
                                }
                            }
                        }
                    } else {
                        let probs = results_map.get(&item.display_id).cloned().unwrap_or_default();
                        aggregated_rows.push((item_name, item.source_data.precedence, probs));
                    }
                    handled_source_data.insert(source_ptr);
                }

                if let Some(agg_trash) = trash_aggregator {
                    aggregated_rows.push(agg_trash);
                }
                aggregated_rows.sort_by_key(|(_, prio, _)| *prio);
                
                print!("{:<15}|{:<6}|", "Item", "Prio");
                for (name, _) in &scenarios {
                    print!("{:<12}|", utils::truncate_string(name, 10));
                }
                println!();

                for (name, prio, probs) in &aggregated_rows {
                    print!("{:<15}| {:<5}|", utils::truncate_string(name, 13), prio);
                    for prob in probs {
                        print!(" {:>10.2}%|", prob * 100.0);
                    }
                    println!();
                }
            }
        }
    }
}