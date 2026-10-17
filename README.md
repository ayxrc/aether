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
It also gives a pretty broken in the developer's words, subwoofer cone with reactive, impulse-driven effects.

That covers the feature set for now.

# Build Instructions

## Prerequisites (Linux)
This project requires the Rust toolchain to build. Download that along with these following basic dependencies.

### Debian and Debian-based systems
```bash
sudo apt update
sudo apt install -y pkg-config libasound2-dev libdbus-1-dev libfontconfig-1-dev libx11-dev libxcb-render0-dev
```

### Arch-based systems
```bash 
sudo pacman -S --needed base-devel pkgconf alsa-lib dbus fontconfig
```

### RHEL/Fedora

```bash
sudo dnf install pkgconfig alsa-lib-devel dbus-devel fontconfig-devel
```


## Building

### - Clone the repo first:

```bash
git clone https://github.com/ayxrc/aether.git 
cd aether
```

### - Test it out:
 
```bash
make run
```

### - Build and install

```bash
sudo make install
```

# Configuration
Aether stores persistent settings in a JSON file at:

- Linux: `$XDG_CONFIG_DIR/aether/config.json`
- Windows: `%APPDATA%\aether\config.json`
- macOS: `~/Library/Application Support/com.ayxrc.aether/`

## Source tree overview
```text
.
├── Cargo.lock
├── Cargo.toml
├── Makefile
├── README.md
└── src
    ├── audio.rs
    ├── doc.rs
    ├── fft.rs
    ├── LICENSE
    ├── lyrics.rs
    ├── main.rs
    ├── mpris.rs
    ├── state.rs
    └── ui.rs
```
     

## License
This project is open-source available under licenses of the GPL.
Kindly credit the author when creating any forks.
