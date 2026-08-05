use nuon::TextJustify;
use std::hash::Hash;

use crate::{
    context::Context,
    song::{PlayerConfig},
};

use super::{neo_btn_icon, state};
use crate::icons;

pub const CARD_W: f32 = 344.0;
pub const CARD_H: f32 = 126.0;

#[derive(Debug)]
enum TrackCardEvent {
    PlayerConfig(PlayerConfig),
    SetVisible(bool),
    Idle,
}

struct TrackCardData {
    track_id: usize,
    track_color_id: usize,
    visible: bool,
    player: PlayerConfig,
    instrument_id: usize,
    is_percussion: bool,
    notes_count: usize,
    hand_info: String,
}

impl super::MenuScene {
    pub fn tracks_page_ui(&mut self, ctx: &mut Context, ui: &mut nuon::Ui) {
        let win_w = ctx.window_state.logical_size.width;
        let win_h = ctx.window_state.logical_size.height;
        let bottom_bar_h = 60.0;

        // --- BOTTOM BAR ---
        nuon::translate().x(0.0).y(win_h).build(ui, |ui| {
            let padding = 10.0;
            let w = 80.0;
            let h = bottom_bar_h;

            nuon::translate().y(-padding - h).add_to_current(ui);

            // Back Button (Left)
            nuon::translate().x(padding).build(ui, |ui| {
                if neo_btn_icon(ui, w, h, icons::left_arrow_icon()) {
                    self.state.go_back();
                }
            });

            // Global Control Buttons (Center)
            if let Some(song) = self.state.song.as_mut() {
                let center_w = 290.0;
                let center_x = (win_w - center_w) / 2.0;

                nuon::translate().x(center_x).build(ui, |ui| {
                    let btn_w = 140.0;
                    let btn_h = 40.0;
                    let btn_y = (bottom_bar_h - btn_h) / 2.0;

                    nuon::translate().y(btn_y).build(ui, |ui| {
                        if nuon::button()
                            .id("all_listen_only")
                            .size(btn_w, btn_h)
                            .label("Listen Only")
                            .color([74, 68, 88])
                            .hover_color([87, 81, 101])
                            .border_radius([6.0; 4])
                            .build(ui)
                        {
                            for track in song.config.tracks.iter_mut() {
                                track.player = PlayerConfig::Auto;
                            }
                        }

                        nuon::translate().x(btn_w + 10.0).add_to_current(ui);

                        if nuon::button()
                            .id("all_play_along")
                            .size(btn_w, btn_h)
                            .label("Play Along")
                            .color([74, 68, 88])
                            .hover_color([87, 81, 101])
                            .border_radius([6.0; 4])
                            .build(ui)
                        {
                            for track in song.config.tracks.iter_mut() {
                                track.player = PlayerConfig::Human;
                            }
                        }
                    });
                });
            }

            // Play Button (Right)
            nuon::translate().x(win_w - w - padding).build(ui, |ui| {
                if neo_btn_icon(ui, w, h, icons::play_icon()) {
                    state::play(&self.state, ctx);
                }
            });
        });

        // --- TRACK CARDS GRID ---
 // --- TRACK CARDS GRID ---
        let track_items: Vec<TrackCardData> = if let Some(song) = self.state.song.as_ref() {
            let mut items: Vec<TrackCardData> = song.file
                .tracks
                .iter()
                .filter(|t| !t.notes.is_empty())
                .map(|track| {
                    let config = &song.config.tracks[track.track_id];
                    let instrument_id = track
                        .programs
                        .last()
                        .map(|p| p.program as usize)
                        .unwrap_or(0);
                    let is_percussion = track.has_drums && !track.has_other_than_drums;

                    let hand_info = if instrument_id <= 7 && !is_percussion {
                        let avg_pitch = if !track.notes.is_empty() {
                            let sum: usize = track.notes.iter().map(|n| n.note as usize).sum();
                            sum / track.notes.len()
                        } else {
                            60
                        };

                        if avg_pitch < 60 {
                            " / Left Hand".to_string()
                        } else {
                            " / Right Hand".to_string()
                        }
                    } else {
                        String::new()
                    };

                    TrackCardData {
                        track_id: track.track_id,
                        track_color_id: track.track_color_id,
                        visible: config.visible,
                        player: config.player,
                        instrument_id,
                        is_percussion,
                        notes_count: track.notes.len(),
                        hand_info,
                    }
                })
                .collect();

            // Sort so the Left Hand card always comes first (left side of the grid)
            items.sort_by(|a, b| {
                let a_is_left = a.hand_info.contains("Left Hand");
                let b_is_left = b.hand_info.contains("Left Hand");
                if a_is_left && !b_is_left {
                    std::cmp::Ordering::Less
                } else if !a_is_left && b_is_left {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            });

            items
        } else {
            Vec::new()
        };
        

        if !track_items.is_empty() {
            let mut events = Vec::new();

            self.tracks_scroll = nuon::scroll()
                .scissor_size(win_w, (win_h - bottom_bar_h).max(0.0))
                .scroll(self.tracks_scroll)
                .build(ui, |ui| {
                    let gap = 14.0;
                    let layout = CardsLayout::new(win_w, track_items.len());
                    let top_margin = 60.0;

                    nuon::translate().y(top_margin).add_to_current(ui);

                    let mut tracks_iter = track_items.iter();

                    loop {
                        let mut end = false;
                        nuon::translate()
                            .x(nuon::center_x(win_w, layout.width))
                            .build(ui, |ui| {
                                for _ in 0..layout.columns {
                                    let Some(data) = tracks_iter.next() else {
                                        end = true;
                                        break;
                                    };

                                    let event = self.track_card(ctx, ui, data);
                                    if !matches!(event, TrackCardEvent::Idle) {
                                        events.push((data.track_id, event));
                                    }

                                    nuon::translate().x(CARD_W + gap).add_to_current(ui);
                                }
                            });

                        nuon::translate().y(CARD_H + gap).add_to_current(ui);

                        if end {
                            break;
                        }
                    }
                });

            // Apply UI events back to song configuration
            if let Some(song) = self.state.song.as_mut() {
                for (track_id, event) in events {
                    match event {
                        TrackCardEvent::PlayerConfig(player) => {
                            song.config.tracks[track_id].player = player;
                        }
                        TrackCardEvent::SetVisible(visible) => {
                            song.config.tracks[track_id].visible = visible;
                        }
                        TrackCardEvent::Idle => {}
                    }
                }
            }
        }
    }

    fn track_card(
        &mut self,
        ctx: &Context,
        ui: &mut nuon::Ui,
        data: &TrackCardData,
    ) -> TrackCardEvent {
        let card_w = CARD_W;
        let card_h = CARD_H;
        let pad = 16.0;
        let track_id = data.track_id;

        let track_color = if !data.visible {
            nuon::Color::new_u8(102, 102, 102, 1.0)
        } else {
            let color_id = data.track_color_id % ctx.config.color_schema().len();
            let color = &ctx.config.color_schema()[color_id].base;
            nuon::Color::new_u8(color.0, color.1, color.2, 1.0)
        };

        let title = if data.is_percussion {
            "Percussion"
        } else {
            midi_file::INSTRUMENT_NAMES[data.instrument_id]
        };

        let subtitle = format!("{} Notes{}", data.notes_count, data.hand_info);

        // 1. Card background
        nuon::quad()
            .size(card_w, card_h)
            .color([37, 35, 42])
            .border_radius([12.0; 4])
            .build(ui);

        let inner_card_w = card_w - pad * 2.0;
        let mut res = TrackCardEvent::Idle;

        nuon::translate().pos(pad, pad).build(ui, |ui| {
            let accent = track_color;
            let accent_hover = nuon::Color::new(
                (accent.r + 0.05).min(1.0),
                (accent.g + 0.05).min(1.0),
                (accent.b + 0.05).min(1.0),
                1.0,
            );

            let regular = nuon::Color::from([74, 68, 88]);
            let regular_hover = nuon::Color::from([87, 81, 101]);

            let icon_size = 36.0;

            // 2. INSTRUMENT ICON (Cached & loaded dynamically)
            let icon_bytes =
                icons::instrument_icon(data.instrument_id, data.is_percussion, &data.hand_info);
            let icon_id = self.get_or_load_icon(ctx, icon_bytes);

            nuon::translate().pos(0.0, 0.0).build(ui, |ui| {
                nuon::image(icon_id)
                    .size(icon_size, icon_size)
                    .build(ui);
            });

            // 3. EYE VISIBILITY TOGGLE BUTTON
            let eye_x = inner_card_w - icon_size;
            nuon::translate().pos(eye_x, 0.0).build(ui, |ui| {
                let eye = if data.visible {
                    icons::eye_icon()
                } else {
                    icons::eye_slash_icon()
                };

                if nuon::button()
                    .id(nuon::Id::hash_with(|h| {
                        track_id.hash(h);
                        "eye_toggle".hash(h);
                    }))
                    .size(icon_size, icon_size)
                    .color([0, 0, 0, 0])
                    .hover_color([255, 255, 255, 20])
                    .icon(eye)
                    .build(ui)
                {
                    res = TrackCardEvent::SetVisible(!data.visible);
                }
            });

            // 4. LABELS (TITLE & SUBTITLE)
            let labels_x = icon_size + 12.0;
            let label_w = eye_x - labels_x - 8.0;

            nuon::translate().x(labels_x).build(ui, |ui| {
                let label_h = 18.0;

                nuon::label()
                    .size(label_w, label_h)
                    .text(title)
                    .text_justify(TextJustify::Left)
                    .font_size(16.0)
                    .build(ui);

                nuon::label()
                    .y(label_h + 2.0)
                    .size(label_w, label_h)
                    .text(&subtitle)
                    .text_justify(TextJustify::Left)
                    .font_size(13.0)
                    .build(ui);
            });

            // 5. PLAYER SEGMENT BUTTONS (Mute / Auto / Human)
            let btn_w = inner_card_w / 3.0;

            nuon::translate().y(46.0).build(ui, |ui| {
                let color = |m: PlayerConfig| {
                    if m == data.player { accent } else { regular }
                };
                let hover_color = |m: PlayerConfig| {
                    if m == data.player {
                        accent_hover
                    } else {
                        regular_hover
                    }
                };

                if nuon::button()
                    .id(nuon::Id::hash_with(|h| {
                        track_id.hash(h);
                        "mute".hash(h);
                    }))
                    .x(0.0)
                    .size(btn_w, 36.0)
                    .color(color(PlayerConfig::Mute))
                    .hover_color(hover_color(PlayerConfig::Mute))
                    .preseed_color(color(PlayerConfig::Mute))
                    .border_radius([8.0, 0.0, 0.0, 8.0])
                    .label("Mute")
                    .build(ui)
                {
                    res = TrackCardEvent::PlayerConfig(PlayerConfig::Mute);
                }

                if nuon::button()
                    .id(nuon::Id::hash_with(|h| {
                        track_id.hash(h);
                        "auto".hash(h);
                    }))
                    .x(btn_w)
                    .size(btn_w, 36.0)
                    .color(color(PlayerConfig::Auto))
                    .hover_color(hover_color(PlayerConfig::Auto))
                    .preseed_color(color(PlayerConfig::Auto))
                    .border_radius([0.0; 4])
                    .label("Auto")
                    .build(ui)
                {
                    res = TrackCardEvent::PlayerConfig(PlayerConfig::Auto);
                }

                if nuon::button()
                    .id(nuon::Id::hash_with(|h| {
                        track_id.hash(h);
                        "human".hash(h);
                    }))
                    .x(btn_w * 2.0)
                    .size(btn_w, 36.0)
                    .color(color(PlayerConfig::Human))
                    .hover_color(hover_color(PlayerConfig::Human))
                    .preseed_color(color(PlayerConfig::Human))
                    .border_radius([0.0, 8.0, 8.0, 0.0])
                    .label("Human")
                    .build(ui)
                {
                    res = TrackCardEvent::PlayerConfig(PlayerConfig::Human);
                }
            });
        });

        res
    }
}

struct CardsLayout {
    columns: u8,
    width: f32,
}

impl CardsLayout {
    fn new(w: f32, tracks_count: usize) -> Self {
        const GAP: f32 = 14.0;

        const LAYOUT_1: f32 = CARD_W;
        const LAYOUT_2: f32 = LAYOUT_1 + GAP + CARD_W;
        const LAYOUT_3: f32 = LAYOUT_2 + GAP + CARD_W;

        let columns = if w > LAYOUT_3 {
            3
        } else if w > LAYOUT_2 {
            2
        } else {
            1
        };

        let columns = columns.min(tracks_count).max(1) as u8;

        Self {
            columns,
            width: match columns {
                3 => LAYOUT_3,
                2 => LAYOUT_2,
                _ => LAYOUT_1,
            },
        }
    }
}