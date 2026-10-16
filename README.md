# Aether
A minimal music player and visualizer built in Rust using `egui` and `rodio`  out of spite and rage out of the lack of music players for Linux with good UI elements. Built with low resource overhead, synchronized (not like Musixmatch) and customizable real-time FFT visualization.
  

# ⚠️ AI Warning
The following source code was troubleshooted, edited and debugged by an Artificial Intelligence robot editing human-written code written by a beginner that copied stackoverflow stuff and misread rust-docs. Be warned.

# ⚠️ Profanity Warning
The following source code may contain comments filled with profanity or hate speech towards certain people. If you are uncomfortable or disgusted by any of it, please inform in issue threads.

## Features
### Minimalistic UI
Aether features a `egui` based UI that is designed to be distraction-free (except the visualizer, the broken subwoofer sim is a design choice sorry). Tested on Hyprland so far.

### FTT-based Visualizer
Aether uses real-time, hardware-accelerated FTT processing with configurable reactivity gain (pretty basic imo but still) with multiple presets.
It also gives a pretty broken in the developer's words, subwoofer cone with reactive, impulse-driven effects.\

That covers the feature set for now.

## Prerequisites (Linux)
This project requires the Rust toolchain to build. Download that along with these following basic dependencies.

### Debian and Debian-based systems
` bash
sudo apt update
sudo apt install -y pkg-config libasound2-dev libdbus-1-dev libfontconfig-1-dev libx11-dev libxcb-render0-dev
`




