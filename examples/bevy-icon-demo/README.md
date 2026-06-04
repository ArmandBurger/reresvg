# bevy-icon-demo

A standalone Bevy app that showcases this resvg fork's SMIL animation feature.
It bakes nine animated SVG icons into sprite sheets and plays them back in a 3×3 grid.

## Run

```bash
cd examples/bevy-icon-demo
cargo run --release
```

## Controls

| Key | Action |
|-----|--------|
| `Space` | pause / resume |
| `.` | step one frame (while paused) |
| `Up` / `Down` | playback speed (FPS) +/- |
| `[` / `]` | frame count - / + (re-bakes; chunky ↔ smooth) |
| `-` / `=` | render size - / + (re-bakes; crispness) |
| `,` | cycle inter-cell padding (0 / 2 / 8 px, re-bakes) |
| `L` | cycle loop mode (loop → ping-pong → once) |
| `B` | cycle background (dark → light → checkerboard) |
| `R` | reset all settings to defaults |
| `Esc` | quit |

## How it works

Each icon is an animated SVG under `assets/icons/`. At startup (and whenever a
re-bake key is pressed) the app calls `resvg::render_sprite_sheet` to sample the
animation into `frame_count` frames packed into a grid, converts the
premultiplied output to straight-alpha RGBA, uploads it as a Bevy texture, and
plays it back through a `TextureAtlas`. This exercises the exact sprite-sheet
export path the fork provides for Bevy.
