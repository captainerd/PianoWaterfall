# PianoWaterfall 🎹💦

Piano learning game. It's a fork of [Neothesia](https://github.com/polymeilex/neothesia) by polymeilex, adding game stats, a MIDI library scene, and various other stuff that fits my personal learning workflow.

 
---

## About This Fork

PianoWaterfall expands upon Neothesia with targeted improvements designed to eliminate friction during practice sessions. This fork is an attempt to bring back the features I had introduced in a previous fork—adapting them as Neothesia transitioned from `iced` to Nuon's custom framework, while incorporating useful additions like MIDI recording. PianoWaterfall essentially serves as an updated vehicle for these personal customizations and a little more. 

*Note: This repository won't be actively maintained beyond what is needed to keep it working for my personal practice sessions.*

---

## What's New in PianoWaterfall?

1. **MIDI Library Scene:** Set your MIDI folder once and instantly browse and select your songs, ready to play.
2. **Game Stats & Streamlined Loop:** No more constant navigating to start and end points. Get immediate game stats to evaluate your performance and quickly loop back to play again.
3. **Theme & Branding Adjustments:** Fully rebranded and customized workspace environment under PianoWaterfall.
4. **Left / Right Hand Info:** Enhanced track details with dedicated icons for each instrument/hand.
5. **Shader Effects & Fireworks:** The original glow effect is enhanced with cool little particle fireworks (fully toggleable in settings).
6. **Chord & Repeat Note Visual Indicator:** Fixed the confusion when reading consecutive overlapping notes (e.g., a short note right after or overlapping a long held note on the same key). Notes that need to be explicitly re-struck now feature a distinct white-glowing indicator right as they hit the keyboard, separating them from notes you just need to keep holding down.
7. **Bug Fixes:** Resolved UI layout scaling bugs introduced by Nuon (such as incorrectly calculated click-region rectangles under XFCE at 720p that were pushing the play screen UI to the left).

---

## Original Features

- Cross-platform MIDI visualizer built in Rust using [WGPU](https://wgpu.rs/).
- Displays music notes from a MIDI file as colorful falling blocks on a virtual piano.
- Designed to bring open-source Synthesia back to life.

---

## Building from Source

Ensure you have Rust (stable) and system dependencies installed.

```bash
# Clone your repository
git clone [https://github.com/YOUR_USERNAME/piano-waterfall.git](https://github.com/YOUR_USERNAME/piano-waterfall.git)
cd piano-waterfall

# Build and run in release mode
cargo build --release
./target/release/piano-watterfall

## Thanks to

- Original Project: [Neothesia](https://github.com/PolyMeilex/Neothesia) by **polymeilex**
- [WGPU](https://wgpu.rs/)
- [Linthesia](https://github.com/linthesia/linthesia)
- [Synthesia](https://github.com/johndpope/pianogame)
