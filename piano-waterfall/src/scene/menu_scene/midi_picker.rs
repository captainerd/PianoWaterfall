use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    scene::menu_scene::{MsgFn, on_async},
    song::Song,
    utils::BoxFuture,
};

use super::UiState;

/// Struct representing a scanned song from your folder
#[derive(Debug, Clone)]
pub struct ScannedSong {
    pub name: String,
    pub path: PathBuf,
}

/// Helper function from original selectsong.rs logic to read a folder for .mid / .midi files
pub fn scan_directory_for_midis(dir_path: &Path) -> Vec<ScannedSong> {
    let mut songs = Vec::new();

    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
                    if extension.eq_ignore_ascii_case("mid")
                        || extension.eq_ignore_ascii_case("midi")
                    {
                        if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                            let clean_name = Song::get_clean_songname(file_name.to_string());
                            songs.push(ScannedSong {
                                name: clean_name,
                                path,
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort alphabetically by song name
    songs.sort_by(|a, b| a.name.cmp(&b.name));
    songs
}

/// 1. Pick a folder using RFD and save it to config
// Inside open_midi_folder_picker (or wherever the async file picker callback handles the result):
pub fn open_midi_folder_picker(_state: &mut UiState) -> BoxFuture<MsgFn> {
    on_async(
        async {
            rfd::AsyncFileDialog::new()
                .set_title("Select MIDI Folder")
                .pick_folder()
                .await
        },
        |folder, _state, ctx| {
            if let Some(folder) = folder {
                let path = folder.path().to_path_buf();

                // Save to config and persist to disk
                ctx.config.set_song_directory(Some(path));
                ctx.config.save(); // <--- Make sure save() is called!
            }
        },
    )
}
/// Pick a single MIDI file directly using RFD
pub fn open_midi_file_picker(data: &mut UiState) -> BoxFuture<MsgFn> {
    data.is_loading = true;
    on_async(open_midi_file_picker_fut(), |res, data, ctx| {
        if let Some((midi, path)) = res {
            ctx.config.set_last_opened_song(Some(path));
            data.song = Some(Song::new(midi));
        }
        data.is_loading = false;
    })
}

async fn open_midi_file_picker_fut() -> Option<(midi_file::MidiFile, PathBuf)> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("midi", &["mid", "midi"])
        .pick_file()
        .await?;

    let path = file.path().to_path_buf();
    let thread_path = path.clone();

    let thread = crate::utils::task::thread::spawn("midi-loader".into(), move || {
        let midi = midi_file::MidiFile::new(&thread_path);
        if let Err(e) = &midi {
            log::error!("{e}");
        }
        midi.map(|midi| (midi, thread_path)).ok()
    });

    thread.join().await.ok().flatten()
}

/// 2. Load a specific MIDI file chosen from the folder list
pub fn load_midi_from_path(data: &mut UiState, song_path: PathBuf) -> BoxFuture<MsgFn> {
    data.is_loading = true;
    on_async(load_midi_file_fut(song_path), |res, data, ctx| {
        if let Some((midi, path)) = res {
            ctx.config.set_last_opened_song(Some(path));
            data.song = Some(Song::new(midi));
        }
        data.is_loading = false;
    })
}

async fn load_midi_file_fut(path: PathBuf) -> Option<(midi_file::MidiFile, PathBuf)> {
    let thread_path = path.clone();
    let thread = crate::utils::task::thread::spawn("midi-loader".into(), move || {
        let midi = midi_file::MidiFile::new(&thread_path);

        if let Err(e) = &midi {
            log::error!("{e}");
        }

        midi.map(|midi| (midi, thread_path)).ok()
    });

    thread.join().await.ok().flatten()
}
