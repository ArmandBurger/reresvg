# reresvg

A personal fork of [resvg](https://github.com/linebender/resvg), an SVG
rendering library, extended with a subset of SMIL animation support and a
sprite-sheet export path for game engines.

This project is based on resvg by the Resvg Authors and is maintained
independently; it follows its own direction and is not affiliated with the
upstream project.

## Crates

- `reresvg` (`crates/resvg`) — the rendering library and `reresvg` CLI binary.
- `usvg` (`crates/usvg`) — SVG parsing and simplification, including the
  animation model `reresvg` renders from.

## Build

```bash
cargo build --release
```

## Animation & sprite-sheet export

`reresvg::render_sprite_sheet` samples an animated SVG into a grid of frames.
See `crates/resvg/examples/animation.rs` for a minimal example, and
`examples/bevy-icon-demo` for a full Bevy app that bakes animated icons into
texture atlases and plays them back.

## License

MIT — see [LICENSE](LICENSE). Portions are derived from resvg and remain
under the original copyright of the Resvg Authors, as noted in the license.
