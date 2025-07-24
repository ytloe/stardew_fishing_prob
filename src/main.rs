//! src/main.rs
//!
//! 程序的主入口。
//! 负责协调各个模块，执行核心流程，并向用户打印最终的概率报告和性能数据。

use std::time::Instant;
use crate::models::ProbabilityDetails; // 引入 ProbabilityDetails
// use itertools::Itertools; // 确保 use itertools

mod config;
mod models;
mod calculator;
mod utils;

fn main() {
    // 1. 启动计时器
    let start_time = Instant::now();

    // 2. 加载所有配置和游戏数据
    let (app_config, game_data) = match config::load_and_build_config() {
        Ok((config, data)) => (config, data),
        Err(e) => {
            eprintln!("\n错误：加载配置失败。\n原因: {}", e);
            return;
        }
    };
    
    // 3. 获取并准备遍历所有 FishAreas
    let location_data = game_data.locations.get(&app_config.location_name)
        .expect("Location data should exist");
    
    let mut fish_area_ids: Vec<Option<String>> = location_data.fish_areas.keys().cloned().map(Some).collect();
    
    let has_default_area_fish = location_data.fish.iter().any(|f| f.fish_area_id.is_none()) 
                             || game_data.locations["Default"].fish.iter().any(|f| f.fish_area_id.is_none());
    
    if has_default_area_fish {
        if !fish_area_ids.contains(&None) {
            fish_area_ids.push(None);
        }
    }

    // 4. 遍历每个水域，并为每个水域生成一个独立的报告块
    for area_id in fish_area_ids {
        let area_name = if let Some(id) = &area_id { id.as_str() } else { "默认" };
        println!("\n地点：{}({})", app_config.location_name, area_name);

        let resolved_items = calculator::get_resolved_fish_list(&app_config, &game_data, &area_id, false);
        
        let time_segments = utils::calculate_time_segments(&resolved_items, &game_data);
        let using_magic_bait = app_config.bait_item_id.as_deref() == Some("(O)908");
        let segments_to_process = if using_magic_bait { vec![(600, 2600)] } else { time_segments };

        for segment in segments_to_process {
            let segment_items = calculator::filter_items_for_time_segment(segment, &resolved_items, &app_config, &game_data);
            
            if !segment_items.is_empty() {
                println!("\n时间段：{:04} - {:04}", segment.0, segment.1);
                
                let detailed_probabilities = calculator::calculate_final_probabilities(&segment_items, &app_config, &game_data);

                // --- 核心修正：在此处实现聚合逻辑 ---
                const TRASH_GROUP_ID: &str = "(O)167|(O)168|(O)169|(O)170|(O)171|(O)172";
                
                let mut display_rows: Vec<ProbabilityDetails> = Vec::new();
                let mut aggregated_trash: Option<ProbabilityDetails> = None;

                for details in detailed_probabilities {
                    if details.source_group_id == TRASH_GROUP_ID {
                        if let Some(agg_trash) = &mut aggregated_trash {
                            // 如果已经有聚合的垃圾项，累加概率
                            agg_trash.final_prob += details.final_prob;
                        } else {
                            // 这是遇到的第一个垃圾项，创建聚合条目
                            aggregated_trash = Some(ProbabilityDetails {
                                display_id: "Trash Group".to_string(),
                                name: "Trash".to_string(),
                                // 其他字段从第一个遇到的垃圾项中复制
                                precedence: details.precedence,
                                get_chance_prob: details.get_chance_prob,
                                bite_chance_prob: details.bite_chance_prob,
                                final_prob: details.final_prob,
                                source_group_id: details.source_group_id,
                            });
                        }
                    } else {
                        // 不是垃圾，直接添加到显示列表
                        display_rows.push(details);
                    }
                }

                // 如果有聚合的垃圾项，将其添加到显示列表
                if let Some(agg_trash) = aggregated_trash {
                    display_rows.push(agg_trash);
                }
                
                // 按最终概率重新排序
                display_rows.sort_by(|a, b| b.final_prob.partial_cmp(&a.final_prob).unwrap_or(std::cmp::Ordering::Equal));
                // --- 聚合逻辑结束 ---

                println!("{:<25} | {:<25} | {:<5} | {:<10} | {:<10} | {}", "ID", "名称", "Prio", "存活概率", "咬钩概率", "最终概率");
                println!("{:-<25}-+-{:-<27}-+-{:-<7}-+-{:-<12}-+-{:-<12}-+-{:-<15}", "", "", "", "", "", "");

                for details in &display_rows { // 使用聚合后的 display_rows
                    println!(
                        "{:<25} | {:<25} | {:<5} | {:>9.2}% | {:>9.2}% | {:>12.2}%",
                        utils::truncate_string(&details.display_id, 23),
                        utils::truncate_string(&details.name, 23),
                        details.precedence,
                        details.get_chance_prob * 100.0,
                        details.bite_chance_prob * 100.0,
                        details.final_prob * 100.0
                    );
                }
            }
        }
    }
    // 5. 停止计时器并打印性能报告
    let duration = start_time.elapsed();
    println!("总计算耗时: {:.2?}", duration);
}