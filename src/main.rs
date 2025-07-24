//! src/main.rs
//!
//! 程序的主入口。
//! 负责协调各个模块，执行核心流程，并向用户打印最终的概率报告。

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
    
    // 检查是否存在无特定区域的鱼，如果有，则加入 None 选项
    let has_default_area_fish = location_data.fish.iter().any(|f| f.fish_area_id.is_none()) 
                             || game_data.locations["Default"].fish.iter().any(|f| f.fish_area_id.is_none());
    
    if has_default_area_fish {
        if !fish_area_ids.contains(&None) {
            fish_area_ids.push(None);
        }
    }

    // 3. 遍历每个水域，并为每个水域生成一个独立的报告块
    for area_id in fish_area_ids {
        let area_name = if let Some(id) = &area_id { id.as_str() } else { "默认" };
        println!("\n地点：{}({})", app_config.location_name, area_name);

        // a. 获取特定水域的物品列表 (这是最关键的修正，所有过滤都在这里完成)
        let resolved_items = calculator::get_resolved_fish_list(&app_config, &game_data, &area_id, false);
        
        // b. 时间分段
        let time_segments = utils::calculate_time_segments(&resolved_items, &game_data);
        let using_magic_bait = app_config.bait_item_id.as_deref() == Some("(O)908");
        let segments_to_process = if using_magic_bait { vec![(600, 2600)] } else { time_segments };

        // c. 遍历时间段计算并打印
        for segment in segments_to_process {
            let segment_items = calculator::filter_items_for_time_segment(segment, &resolved_items, &app_config, &game_data);
            
            if !segment_items.is_empty() {
                println!("\n时间段：{:04} - {:04}", segment.0, segment.1);
                
                // --- 调用全新的确定性概率计算函数 ---
                let detailed_probabilities = calculator::calculate_final_probabilities(&segment_items, &app_config, &game_data);

                println!("{:<25} | {:<25} | {:<5} | {:<10} | {:<10} | {}", "ID", "名称", "Prio", "存活概率", "咬钩概率", "最终概率");
                println!("{:-<25}-+-{:-<27}-+-{:-<7}-+-{:-<12}-+-{:-<12}-+-{:-<15}", "", "", "", "", "", "");
                
                // --- 修正：为每个时间段独立计算总概率 ---
                let mut total_prob_sum_for_segment = 0.0;
                for details in &detailed_probabilities {
                    total_prob_sum_for_segment += details.final_prob;
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
                println!("\n本时段总概率: {:.2}%", total_prob_sum_for_segment * 100.0);
            }
        }
    }
}
