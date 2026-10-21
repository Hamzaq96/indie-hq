---
name: Rooms and faces
overview: "Teach a small client-only upgrade in `crates/client/src/main.rs`: draw the three zones as boxed rooms with a walkable door gap, and pick a stable one-cell face from each player id."
todos:
  - id: map-grid
    content: Replace the Block and full-height pipes with a 30x12 box-drawing grid and door gaps at y 4..=6
    status: pending
  - id: collision
    content: Add is_wall and reject moves onto wall cells in on_key
    status: pending
  - id: glyphs
    content: Add face_for(id) and draw it for self and others instead of @ and o
    status: pending
isProject: false
---

# Boxed rooms and face glyphs

All of this stays in [`crates/client/src/main.rs`](crates/client/src/main.rs). Zone rules stay as they are: `x < 10` Focus, `x < 20` Open, otherwise Office — same as [`crates/protocol/src/lib.rs`](crates/protocol/src/lib.rs) `zone_at`. No protocol or server change.

## Why the current map looks like strips

`draw` paints a `Block` border, then a full-height `|` at `BOUND1` (10) and `BOUND2` (20). Those columns are also walkable, so the avatar covers the wall. A room needs the same columns to be walls, with a gap you can pass through.

Interior cells are already `0..27` by `0..9` (`MAX_X` / `MAX_Y`). Column 10 and column 20 are the shared walls. A door is a few `y` values on those columns that are not walls (use `4..=6`). Standing in the door at `x == 10` still counts as Open, which matches today’s `zone_at`.

## Rooms

Drop the bordered `Block` for the map so you do not get a second frame around your own walls. Build one 30×12 character grid and render it with a single `Paragraph`.

Row shape (30 columns):

- col 0: left border (`┌` / `│` / `└`)
- cols 1–10: Focus (interior `x` 0–9)
- col 11: Focus/Open wall (interior `x` 10) — `┬` / `│` / `┴`, or a space on door rows
- cols 12–20: Open
- col 21: Open/Office wall (interior `x` 20)
- cols 22–28: Office
- col 29: right border (`┐` / `│` / `┘`)

Put `FOCUS`, `OPEN`, and `OFFICE` on the first interior row of each room. Draw players after the map so a glyph covers a letter if someone stands on it.

Add `is_wall(x, y) -> bool`: true only on the two divider columns, false inside the door range. In `on_key`, compute the next `(x, y)` first; if that cell is a wall, return `false` so no `Move` is sent. `w`/`a` already stop at 0; keep the existing `MAX_X` / `MAX_Y` clamps.

## Faces

Terminal cells are one column wide. Emoji often take two columns and will shove the map sideways. Use a fixed list of width-1 glyphs, for example `☺ ☻ ♠ ♣ ♥ ♦ ▲ ●`.

```rust
fn face_for(id: &str) -> char {
    const FACES: &[char] = &['☺', '☻', '♠', '♣', '♥', '♦', '▲', '●'];
    let n = id.bytes().fold(0usize, |acc, b| acc.wrapping_add(b as usize));
    FACES[n % FACES.len()]
}
```

`others` is already keyed by id, so hash that key. For yourself, hash `app.id` once `Welcome` arrives; before that, draw `☺`. Same id always gets the same face, including across two clients.

## Check

`cargo build`, then `cargo run -p client`. Walk left and right: walls should stop you, the gap should let you change rooms, and the title zone should flip when `x` crosses 10 and 20. A second client should show a different face than yours.
