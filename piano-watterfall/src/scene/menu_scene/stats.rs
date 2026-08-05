use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::{
    context::Context,
    scene::menu_scene::{icons, neo_btn_icon, state},
    song::Song,
};

pub const ROW_H: f32 = 44.0;
pub const TABLE_W: f32 = 800.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedStats {
    pub date: DateTime<Utc>,
    pub notes_hit: usize,
    pub slow_hits: usize,
    pub wrong_notes: usize,
    pub correct_note_times: usize,
}

impl SavedStats {
    pub fn delete_for_song(song_name: &str) {
        if let Some(path) = Self::get_file_path(song_name) {
            let _ = fs::remove_file(path);
        }
    }

    pub fn score_cooking(&self) -> usize {
        let total_attempts = self.notes_hit + self.wrong_notes + self.slow_hits;
        if total_attempts == 0 {
            return 0;
        }
        let accuracy = (self.notes_hit as f32 / total_attempts as f32) * 100.0;
        accuracy.round() as usize
    }

    fn get_file_path(song_name: &str) -> Option<PathBuf> {
        let proj_dirs = directories::ProjectDirs::from("", "", "piano_watterfall")?;
        let data_dir = proj_dirs.data_dir();
        fs::create_dir_all(data_dir).ok()?;
        let safe_name = song_name.chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect::<String>();
        Some(data_dir.join(format!("{}_stats.json", safe_name)))
    }

    pub fn load_for_song(song_name: String) -> Vec<SavedStats> {
        let Some(path) = Self::get_file_path(&song_name) else {
            return Vec::new();
        };
        
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(mut stats) = serde_json::from_str::<Vec<SavedStats>>(&data) {
                stats.sort_by(|a, b| b.score_cooking().cmp(&a.score_cooking()));
                return stats;
            }
        }
        Vec::new()
    }

    pub fn save_for_song(&self, song_name: &str) {
        let Some(path) = Self::get_file_path(song_name) else {
            return;
        };

        let mut stats = Self::load_for_song(song_name.to_string());
        stats.push(self.clone());
        
        if let Ok(data) = serde_json::to_string(&stats) {
            let _ = fs::write(path, data);
        }
    }
}

impl super::MenuScene {
    pub fn stats_page_ui(&mut self, ctx: &mut Context, ui: &mut nuon::Ui) {
        let win_w = ctx.window_state.logical_size.width;
        let win_h = ctx.window_state.logical_size.height;
        let bottom_bar_h = 60.0;
        let top_header_h = 70.0;

        let song_name = if let Some(song) = self.state.song() {
            Song::get_clean_songname(song.file.name.clone())
        } else {
            "No Song Selected".to_string()
        };

        nuon::translate().x(0.0).y(20.0).build(ui, |ui| {
            nuon::label()
                .text(&song_name)
                .size(win_w, 40.0)
                .font_size(22.0)
                .bold(true)
                .text_justify(nuon::TextJustify::Center)
                .build(ui);
        });

        let columns = [
            ("Place", 80.0),
            ("Date", 170.0),
            ("Score", 100.0),
            ("Good Hits", 100.0),
            ("Slow Hits", 100.0),
            ("Wrong Notes", 110.0),
            ("Good Durations", 140.0),
        ];

        let start_x = nuon::center_x(win_w, TABLE_W);

        nuon::translate().x(start_x).y(top_header_h).build(ui, |ui| {
            nuon::quad()
                .size(TABLE_W, ROW_H)
                .color([45, 42, 55])
                .border_radius([8.0; 4])
                .build(ui);

            let mut cur_x = 0.0;
            for (header, width) in columns {
                nuon::translate().x(cur_x).build(ui, |ui| {
                    nuon::label()
                        .text(header)
                        .size(width, ROW_H)
                        .font_size(14.0)
                        .bold(true)
                        .color([200, 200, 220])
                        .text_justify(nuon::TextJustify::Center)
                        .build(ui);
                });
                cur_x += width;
            }
        });

        let sorted_stats = SavedStats::load_for_song(song_name.clone());

        let list_y = top_header_h + ROW_H + 10.0;
        let list_h = (win_h - list_y - bottom_bar_h - 10.0).max(100.0);

        nuon::translate().x(start_x).y(list_y).build(ui, |ui| {
            self.stats_scroll = nuon::scroll()
                .scissor_size(TABLE_W, list_h)
                .scroll(self.stats_scroll)
                .build(ui, |ui| {
                    if sorted_stats.is_empty() {
                        nuon::label()
                            .text("No scores recorded yet. Play the song to set a score!")
                            .size(TABLE_W, 60.0)
                            .font_size(16.0)
                            .color([160, 160, 180])
                            .text_justify(nuon::TextJustify::Center)
                            .build(ui);
                    } else {
                        let row_gap = 6.0;
                        for (index, stats) in sorted_stats.iter().enumerate() {
                            let score = stats.score_cooking();
                            let datetime: DateTime<Local> = stats.date.with_timezone(&Local);
                            let date_str = datetime.format("%d/%m/%y %H:%M").to_string();

                            let place_str = match index {
                                0 => "1st".to_string(),
                                1 => "2nd".to_string(),
                                2 => "3rd".to_string(),
                                _ => format!("{}th", index + 1),
                            };

                            let bg_color = if index % 2 == 0 {
                                [32, 30, 40]
                            } else {
                                [26, 24, 34]
                            };

                            nuon::translate().y((ROW_H + row_gap) * index as f32).build(ui, |ui| {
                                nuon::quad()
                                    .size(TABLE_W, ROW_H)
                                    .color(bg_color)
                                    .border_radius([6.0; 4])
                                    .build(ui);

                                let row_vals = [
                                    (place_str, 80.0),
                                    (date_str, 170.0),
                                    (score.to_string(), 100.0),
                                    (stats.notes_hit.to_string(), 100.0),
                                    (stats.slow_hits.to_string(), 100.0),
                                    (stats.wrong_notes.to_string(), 110.0),
                                    (stats.correct_note_times.to_string(), 140.0),
                                ];

                                let mut cur_x = 0.0;
                                for (val, width) in row_vals {
                                    nuon::translate().x(cur_x).build(ui, |ui| {
                                        nuon::label()
                                            .text(&val)
                                            .size(width, ROW_H)
                                            .font_size(14.0)
                                            .color([230, 230, 245])
                                            .text_justify(nuon::TextJustify::Center)
                                            .build(ui);
                                    });
                                    cur_x += width;
                                }
                            });
                        }
                    }
                });
        });

        // Single unified bottom bar containing Back, Delete Stats, and Play buttons
        nuon::translate().x(0.0).y(win_h).build(ui, |ui| {
            nuon::translate().y(-10.0).add_to_current(ui);
            nuon::translate().y(-bottom_bar_h).add_to_current(ui);

            let gap = 10.0;
            let w = 80.0;
            let h = bottom_bar_h;

            // Back button
            nuon::translate().x(gap).build(ui, |ui| {
                if neo_btn_icon(ui, w, h, icons::left_arrow_icon()) {
                    self.state.go_back();
                }
            });

            // Trash/Clean stats button
            nuon::translate().x(gap * 2.0 + w).build(ui, |ui| {
                if neo_btn_icon(ui, w, h, icons::trash_icon()) {
                    SavedStats::delete_for_song(&song_name);
                }
            });

            // Play button (if song exists)
            if self.state.song().is_some() {
                nuon::translate().x(win_w - w - gap).build(ui, |ui| {
                    if neo_btn_icon(ui, w, h, icons::play_icon()) {
                        state::play(&self.state, ctx);
                    }
                });
            }
        });
    }
}