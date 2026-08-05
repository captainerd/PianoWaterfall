use crate::{
    context::Context,
    scene::menu_scene::{icons, neo_btn_icon, state},
    scene::menu_scene::state::Page,
};

impl super::MenuScene {
    pub fn song_selection_page_ui(&mut self, ctx: &mut Context, ui: &mut nuon::Ui) {
        let win_w = ctx.window_state.logical_size.width;
        let win_h = ctx.window_state.logical_size.height;

        let panel_w = 750.0;
        let panel_h = 540.0;
        let bottom_bar_h = 60.0;
        let item_h = 32.0;

        let mut song_file_name = String::new();
        if let Some(path_buf) = ctx.config.last_opened_song() {
            if let Some(file_name) = path_buf.file_name() {
                if let Some(name) = file_name.to_str() {
                    song_file_name = crate::song::Song::get_clean_songname(name.to_string());
                }
            }
        }

        let mut song_directory = String::new();
        if let Some(path_buf) = ctx.config.song_directory() {
            song_directory = path_buf.to_string_lossy().to_string();
        }

        // --- BOTTOM BAR ---
   
        nuon::translate().x(0.0).y(win_h).build(ui, |ui| {
            let padding = 10.0;
            let w = 80.0;
            let h = bottom_bar_h;

            nuon::translate().y(-padding).add_to_current(ui);
            nuon::translate().y(-h).add_to_current(ui);

            // Back Button (Left) with icon
            nuon::translate().x(padding).build(ui, |ui| {
                if neo_btn_icon(ui, w, h, icons::left_arrow_icon()) {
                    self.state.go_back();
                }
            });

            // Change Folder Button (Center) with Bootstrap folder icon
            let folder_btn_w = 80.0;
            nuon::translate().x((win_w - folder_btn_w) / 2.0).build(ui, |ui| {
                if neo_btn_icon(ui, folder_btn_w, h, icons::folder_icon()) {
                    self.futures.push(super::midi_picker::open_midi_folder_picker(&mut self.state));
                }
            });

            // Play/Track Selection Button (Right - Only shown if a song is loaded)
            if self.state.song().is_some() {
                nuon::translate().x(win_w - w - padding).build(ui, |ui| {
                    if neo_btn_icon(ui, w, h, icons::play_icon()) {
                        self.state.go_to(Page::TrackSelection);
                    }
                });
            }
        });

        nuon::translate()
            .x(nuon::center_x(win_w, panel_w))
            .y(nuon::center_y(win_h, panel_h))
            .build(ui, |ui| {
                // Title
                nuon::label()
                    .text("Song Library")
                    .font_size(24.0)
                    .size(panel_w, 30.0)
                    .build(ui);

                // Selected Song subtitle
                nuon::translate().y(30.0).add_to_current(ui);
                nuon::label()
                    .text(format!("Selected song: {}", if song_file_name.is_empty() { "None" } else { &song_file_name }))
                    .font_size(11.0)
                    .size(panel_w, 20.0)
                    .build(ui);

                // Song list area container offset
                nuon::translate().y(60.0).add_to_current(ui);

                let songs = if let Some(dir) = ctx.config.song_directory() {
                    super::midi_picker::scan_directory_for_midis(dir)
                } else {
                    Vec::new()
                };

                if songs.is_empty() {
                    nuon::label()
                        .text("No MIDI files found. Click 'Folder' below to choose a directory.")
                        .font_size(14.0)
                        .size(panel_w, 120.0)
                        .build(ui);
                } else {
                    nuon::scroll().build(ui, |ui| {
                        for song_info in songs {
                            let clean_name = crate::song::Song::get_clean_songname(song_info.name.clone());
                            let is_selected = song_file_name == clean_name;

                            let clicked = super::list_btn::list_btn()
                                .size(panel_w - 20.0, item_h)
                                .label(&clean_name)
                                .font_size(13.0)
                                .selected(is_selected)
                                .color(nuon::Color::new_u8(230, 230, 230, 1.0))
                                .build(ui);

                            if clicked {
                                self.futures.push(super::midi_picker::load_midi_from_path(
                                    &mut self.state,
                                    song_info.path,
                                ));
                            }
                            nuon::translate().y(item_h + 4.0).add_to_current(ui);
                        }
                    });
                }

                // Directory Path Footer
                nuon::translate().y(panel_h - 20.0).build(ui, |ui| {
                    nuon::label()
                        .text(format!("Path: {}", song_directory))
                        .font_size(10.0)
                        .size(panel_w, 20.0)
                        .build(ui);
                });
            });
    }
}