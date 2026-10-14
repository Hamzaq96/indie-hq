# indie-hq

A terminal coworking space for indie hackers. You walk an avatar around a small HQ, see who else is online, and talk to the people standing in the same zone.

Voice is a Discord invite for the zone you are in. Office hours is a Cal.com link. There is no custom voice or payment flow in this spike.

## What you see

The map is three vertical strips:

| Zone | Where | What it is for |
| --- | --- | --- |
| Focus | left | quiet work |
| Open | middle | general coworking |
| Office | right | office hours |

`@` is you. `o` is someone else. Chat under the map is limited to your current zone: the server tags each message from your position and only delivers it to people standing in that same strip.

## Run it

You need a recent Rust toolchain.

```bash
cargo build
cargo run -p server
```

The server listens on `ws://127.0.0.1:3000/ws`. In other terminals:

```bash
cargo run -p client
```

Open two clients to see both avatars and to try zone chat. Walk one into a different strip before sending a message. The client who left the zone should not see it.

Discord and Cal links in the client are placeholders (`discord.gg/placeholder-*` and `cal.com/placeholder-calendar`). Replace them in `crates/client/src/main.rs` before using those keys for real.

## Keys

| Key | Action |
| --- | --- |
| `wasd` or arrow keys | move |
| `Shift+C` | start a chat line |
| `Enter` | send the line |
| `Esc` | discard the draft |
| `Backspace` | delete a character while typing |
| `Shift+V` | open the Discord invite for your current zone |
| `Shift+B` | open the Cal link while you are in Office |
| `q` | quit |

Movement keys do nothing while you are typing.

## Layout

```
indie-hq/
  Cargo.toml            workspace
  crates/protocol/      shared ClientMessage / ServerMessage (serde JSON)
  crates/server/        axum WebSocket server
  crates/client/        ratatui TUI
```

`protocol` is the shared contract. Both the server and the client depend on it, and a `Zone` on a chat message is chosen by the server from the player's stored position.
