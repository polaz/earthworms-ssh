# worms//ssh

A multiplayer artillery game where the lobby is an SSH server, the arena is ANSI text, and your worm is whatever username you typed at `ssh`. No accounts, no goals, no leaderboards that matter, no point. Just blow up other people's pixels for a few minutes.

## Try it

Browser: **<https://worms.private.systems>**

SSH:

```sh
ssh -p 1025 <yourname>@worms.private.systems
```

Either route drops you into the same shared world. Authentication accepts `none`, the username you typed becomes the name floating over your worm, and you are dropped into the same shared world as everyone else currently connected. Quit with `Ctrl-C`. Be nice. Or don't.

The server tries to give you the best output your terminal can handle: truecolor if `COLORTERM=truecolor`, otherwise 256, then 16, then monochrome for `TERM=dumb`. On entry it asks once whether you have a Powerlevel10k-style Nerd Font; press `y` for icon glyphs or just Enter for ASCII.

## Controls

| Key | Action |
| --- | --- |
| `A` / `D` or `←` / `→` | Move left / right (auto step-up over 1-pixel ledges) |
| `Space` | Jump (height grows with horizontal velocity) |
| `W` / `S` or `↑` / `↓` (or `J` / `L`) | Aim up / down, range `-8..+8` (each step = 11.25°) |
| `1` / `2` | Bazooka / Grenade |
| `Enter` (hold) | Charge a shot. Release auto-fires 0.5s after you stop tapping |
| `Ctrl-C` | Leave |

Power grows 5% every 0.5s while you keep tapping `Enter`, capped at 100% over 3s. A single tap gives roughly 15%. At 100% power and a 45° aim the projectile crosses the whole map. Direct hits at your own feet take about 80% of your HP, so you can yeet yourself across the arena with grenades, but you cannot one-shot yourself.

Arrow keys are decoded for any terminal flavor your client throws at the wire (CSI, SS3, modified arrows). The session also resets `DECCKM` so cursor key encoding is deterministic.

## World rules

The arena is one shared 308x72 physics grid sampled into your terminal viewport with 2x2 sub-cell anti-aliasing, so slopes render with diagonal block characters instead of staircase teeth. Terrain has surface tension that is not 100 percent: slopes steeper than the talus angle slough off, and a single tall column of dirt eventually collapses into a polite pile.

Earth occupies at most 50 percent of the grid. Below that, new dirt extrudes from the bottom in 1 to 3 pixel bursts, faster the emptier the map gets, slower as it approaches the cap. Anything left unsupported in mid-air hangs for about a second, then falls one pixel per tick. If you get fully buried with no air pocket above, your worm dies and respawns after three seconds, same as any other death.

Floating HP and Power gauges hover next to each worm: HP on the left (color shifts from green to yellow to red as you bleed out), Power on the right while charging. The aim crosshair only appears for your own worm, drawn in the direction you are facing.

## Build and run locally

```sh
ssh-keygen -q -t ed25519 -N '' -f host_key
cargo run --release -- --listen 0.0.0.0:2222 --host-key host_key
ssh -p 2222 your_name@127.0.0.1
```

For two local sessions without polluting your real SSH config:

```sh
cargo run -- --listen 127.0.0.1:22345 --host-key host_key --seed 7
ssh -F ssh_config.local -l alice worms-local
ssh -F ssh_config.local -l bob   worms-local
```

## Architecture

`russh` (pure Rust crypto) hosts the SSH protocol in-process. `src/ssh.rs` accepts `none` auth, translates PTY channel input into engine events, and streams ANSI frames back. `src/game.rs` owns all mutable simulation state in one authoritative 20Hz tick. `src/render.rs` projects the shared world into a viewport-sized ANSI frame for each client, with incremental updates that only resend changed rows.

## Verify

```sh
cargo fmt --all -- --check
cargo nextest run
cargo clippy --all-targets -- -D warnings
```

## Why

Because typing `ssh -p 1025 something@somewhere` and getting a tiny pixel war is funnier than the time it took to write it. That is the whole reason.
