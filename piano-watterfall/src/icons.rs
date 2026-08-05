#![allow(unused)]

pub fn cone_icon() -> &'static str { "\u{F2D2}" }
pub fn gear_icon() -> &'static str { "\u{F3E5}" }
pub fn gear_fill_icon() -> &'static str { "\u{F3E2}" }
pub fn repeat_icon() -> &'static str { "\u{f130}" }
pub fn play_icon() -> &'static str { "\u{f4f4}" }
pub fn pause_icon() -> &'static str { "\u{f4c3}" }
pub fn left_arrow_icon() -> &'static str { "\u{f12f}" }
pub fn minus_icon() -> &'static str { "\u{F2EA}" }
pub fn folder_icon() -> &'static str { "\u{F330}" }
pub fn plus_icon() -> &'static str { "\u{F4FE}" }
pub fn balloon_icon() -> &'static str { "\u{f709}" }
pub fn note_list_icon() -> &'static str { "\u{f49f}" }
pub fn caret_down() -> &'static str { "\u{f229}" }
pub fn record_icon() -> &'static str { "\u{f519}" }
pub fn record_stop_icon() -> &'static str { "\u{f591}" }
pub fn trash_icon() -> &'static str { "\u{F5DE}" }
pub fn trash_fill_icon() -> &'static str { "\u{F5DD}" }
pub fn save_icon() -> &'static str { "\u{f7D9}" }
pub fn film_icon() -> &'static str { "\u{F3C3}" } //

// Toggle Icons (Unicode eye symbols if font available, or fallback strings)
pub fn eye_icon() -> &'static str { "\u{F341}" }
pub fn eye_slash_icon() -> &'static str { "\u{F340}" }

// Embedded PNG Image Bytes
pub const TOGGLE_ON: &[u8] = include_bytes!("img/toggle_on.png");
pub const TOGGLE_OFF: &[u8] = include_bytes!("img/toggle_off.png");

pub const PIANO_LEFT: &[u8] = include_bytes!("img/piano-left.png");
pub const PIANO_RIGHT: &[u8] = include_bytes!("img/piano-right.png");

// Instrument icons
pub const GUITARS: &[u8] = include_bytes!("img/guitars.png");
pub const BRASSES: &[u8] = include_bytes!("img/brasses.png");
pub const PERCUSSIONS: &[u8] = include_bytes!("img/percussions.png");
pub const FLUTES: &[u8] = include_bytes!("img/flutes.png");
pub const CHOIRS: &[u8] = include_bytes!("img/choirs.png");
pub const VIOLINS: &[u8] = include_bytes!("img/violins.png");
pub const BELLS: &[u8] = include_bytes!("img/bells.png");
pub const XYLOPHONES: &[u8] = include_bytes!("img/xylophones.png");
pub const UNCATEGORIZED: &[u8] = include_bytes!("img/uncategorized.png");

/// Returns the matching PNG icon bytes for a MIDI instrument program ID
pub fn instrument_icon(program_id: usize, is_percussion: bool, hand_info: &str) -> &'static [u8] {
    if is_percussion {
        return PERCUSSIONS;
    }
    if hand_info.contains("Left") {
        return PIANO_LEFT;
    }
    if hand_info.contains("Right") {
        return PIANO_RIGHT;
    }

    match program_id {
        0..=7 => PIANO_RIGHT,
        8..=15 => BELLS,
        24..=31 => GUITARS,
        40..=47 => VIOLINS,
        52..=55 => CHOIRS,
        56..=63 => BRASSES,
        72..=79 => FLUTES,
        _ => UNCATEGORIZED,
    }
}