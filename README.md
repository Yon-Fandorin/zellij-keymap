# zellij-keymap

A mode-aware keybinding viewer plugin for [Zellij](https://zellij.dev/).

Unlike `zellij-forgot` (static list) or `zjstatus-hints` (default keys only, statusbar-bound), this plugin:

- Reads your **actual custom keybindings** from the running Zellij config
- **Highlights the current mode's shortcuts** at the top, updated in real-time as you switch modes
- Opens as a **standalone floating panel** with search and scroll
- Applies **theme colors dynamically** from your Zellij palette

## Install

Download `zellij_keymap.wasm` from the [latest release](https://github.com/Yon-Fandorin/zellij-keymap/releases/latest) and place it in your Zellij plugins directory:

```bash
mkdir -p ~/.config/zellij/plugins
cp zellij_keymap.wasm ~/.config/zellij/plugins/
```

## Usage

Add a keybinding to your `config.kdl`:

```kdl
shared_except "locked" {
    bind "Alt /" {
        LaunchOrFocusPlugin "file:~/.config/zellij/plugins/zellij_keymap.wasm" {
            floating true
        }
    }
}
```

Then press `Alt+/` to open the keymap panel.

## Controls

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `Ctrl+d` | Half page down |
| `Ctrl+u` | Half page up |
| `g` | Go to top |
| `G` | Go to bottom |
| `/` | Search |
| `Esc` / `q` | Close |

## Build from source

```bash
rustup target add wasm32-wasip1
cargo build --release
cp target/wasm32-wasip1/release/zellij-keymap.wasm ~/.config/zellij/plugins/zellij_keymap.wasm
```

## License

MIT
