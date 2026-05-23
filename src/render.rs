use std::fmt::Write;

use crate::game::{
    ColorDepth, Game, GlyphMode, HEIGHT, MAX_HEALTH, PlayerId, Tile, WIDTH, WORLD_SCALE, Weapon,
};

const MAX_VIEWPORT_WIDTH: usize = 300;
const MAX_VIEWPORT_HEIGHT: usize = 120;

pub fn frame(
    game: &Game,
    viewer: PlayerId,
    columns: u32,
    rows: u32,
    colors: ColorDepth,
    glyphs: GlyphMode,
) -> String {
    let terminal_width = (columns as usize).saturating_sub(1).max(6);
    let feed_lines = usize::from(rows >= 10) * 2;
    let viewport_width = terminal_width
        .saturating_sub(2)
        .clamp(4, MAX_VIEWPORT_WIDTH);
    let viewport_height = (rows as usize)
        .saturating_sub(4 + feed_lines)
        .clamp(2, MAX_VIEWPORT_HEIGHT);
    let mut canvas = vec![vec![' '; viewport_width]; viewport_height];
    let mut overlay: Vec<Vec<Option<Style>>> = vec![vec![None; viewport_width]; viewport_height];

    for (sy, row) in canvas.iter_mut().enumerate() {
        for (sx, cell) in row.iter_mut().enumerate() {
            *cell = sampled_terrain(game, sx, sy, viewport_width, viewport_height);
        }
    }

    for projectile in &game.projectiles {
        plot_world(
            &mut canvas,
            projectile.x.round() as i32,
            projectile.y.round() as i32,
            match glyphs {
                GlyphMode::Ascii => projectile.glyph,
                GlyphMode::Powerlevel10k => '\u{f135}',
            },
        );
    }
    for blast in &game.blasts {
        let pulse = match glyphs {
            GlyphMode::Ascii if blast.age % 2 == 0 => '@',
            GlyphMode::Ascii => '*',
            GlyphMode::Powerlevel10k => '\u{f0e7}',
        };
        plot_world(&mut canvas, blast.x, blast.y, pulse);
        plot_world(&mut canvas, blast.x - 1, blast.y, pulse);
        plot_world(&mut canvas, blast.x + 1, blast.y, pulse);
        plot_world(&mut canvas, blast.x, blast.y - 1, pulse);
    }
    for player in game.players.values() {
        if player.respawn_ticks == 0 {
            let tail = match (glyphs, player.facing >= 0) {
                (GlyphMode::Ascii, true) => '>',
                (GlyphMode::Ascii, false) => '<',
                (GlyphMode::Powerlevel10k, true) => '\u{e0b0}',
                (GlyphMode::Powerlevel10k, false) => '\u{e0b2}',
            };
            let worm = match glyphs {
                GlyphMode::Ascii if player.id == viewer => '@',
                GlyphMode::Ascii => 'w',
                GlyphMode::Powerlevel10k if player.id == viewer => '\u{f188}',
                GlyphMode::Powerlevel10k => '\u{f2db}',
            };
            if player.id == viewer {
                let aim_mag = i32::from(player.aim).abs();
                let horiz = (WORLD_SCALE as i32 * 4)
                    .saturating_sub(aim_mag * WORLD_SCALE as i32 / 4)
                    .max(WORLD_SCALE as i32);
                plot_world(
                    &mut canvas,
                    player.x.round() as i32 + player.facing as i32 * horiz,
                    player.y.round() as i32 + i32::from(player.aim) * WORLD_SCALE as i32 * 3 / 4,
                    match glyphs {
                        GlyphMode::Ascii => '+',
                        GlyphMode::Powerlevel10k => '\u{f140}',
                    },
                );
            }
            plot_world(
                &mut canvas,
                player.x.round() as i32 + player.facing as i32 * WORLD_SCALE as i32,
                player.y.round() as i32,
                tail,
            );
            plot_world(
                &mut canvas,
                player.x.round() as i32,
                player.y.round() as i32,
                worm,
            );
            let hp_value = ((player.health.max(0) as i32 * 16 + MAX_HEALTH as i32 / 2)
                / MAX_HEALTH as i32)
                .clamp(0, 16);
            let hp_pct = player.health.max(0) as i32 * 100 / MAX_HEALTH as i32;
            let hp_style = if hp_pct >= 60 {
                Style::Health
            } else if hp_pct >= 25 {
                Style::Aim
            } else {
                Style::Weapon
            };
            let wx = player.x.round() as i32;
            let wy = player.y.round() as i32;
            let cw = canvas.first().map_or(0, Vec::len) as i32;
            let ch_h = canvas.len() as i32;
            let cwx = wx * cw / WIDTH as i32;
            let cwy = wy * ch_h / HEIGHT as i32;
            let (bottom, top) = level_split(hp_value);
            plot_canvas_styled(
                &mut canvas,
                &mut overlay,
                cwx - 1,
                cwy - 1,
                level_char(bottom),
                hp_style,
            );
            plot_canvas_styled(
                &mut canvas,
                &mut overlay,
                cwx - 1,
                cwy - 2,
                level_char(top),
                hp_style,
            );
            if let Some(start) = player.charge_started {
                let elapsed = game.tick_number.saturating_sub(start).min(40);
                let percent = ((elapsed / 2) * 5).min(100) as i32;
                let pw_value = (percent * 16 / 100).clamp(0, 16);
                let (bottom, top) = level_split(pw_value);
                plot_canvas_styled(
                    &mut canvas,
                    &mut overlay,
                    cwx + 1,
                    cwy - 1,
                    level_char(bottom),
                    Style::Weapon,
                );
                plot_canvas_styled(
                    &mut canvas,
                    &mut overlay,
                    cwx + 1,
                    cwy - 2,
                    level_char(top),
                    Style::Weapon,
                );
            }
        }
    }

    let mut out = String::with_capacity(viewport_width * (viewport_height + 8));
    out.push_str("\x1b[H");
    if let Some(me) = game.players.get(&viewer) {
        let weapon = match me.weapon {
            Weapon::Bazooka => "BAZOOKA [1]",
            Weapon::Grenade => "GRENADE [2]",
        };
        let state = if me.respawn_ticks > 0 {
            format!("RESPAWN {:.1}s", me.respawn_ticks as f32 / 20.0)
        } else if let Some(start) = me.charge_started {
            let elapsed = game.tick_number.saturating_sub(start).min(40);
            let percent = ((elapsed / 2) * 5).min(100) as u8;
            format!("CHARGE {percent:>3}%")
        } else {
            format!(
                "HP {:>3}%",
                me.health.max(0) as i32 * 100 / MAX_HEALTH as i32
            )
        };
        push_terminal_line(
            &mut out,
            &format!(
                "WORMS//SSH {} {} {} VX:{:+.1} AIM:{:+} K:{} D:{} P:{}",
                me.name,
                state,
                weapon,
                me.vx,
                me.aim,
                me.kills,
                me.deaths,
                game.players.len()
            ),
            terminal_width,
        );
    }
    out.push('+');
    out.push_str(&"-".repeat(viewport_width));
    out.push_str("+\x1b[K\r\n");
    for (row, overlay_row) in canvas.iter().zip(overlay.iter()) {
        out.push('|');
        let mut active_style = Style::Plain;
        for (ch, override_style) in row.iter().zip(overlay_row.iter()) {
            let style = override_style.unwrap_or_else(|| style_for(*ch, glyphs));
            if style != active_style {
                out.push_str(style.escape(colors));
                active_style = style;
            }
            out.push(*ch);
        }
        out.push_str("\x1b[0m|\x1b[K\r\n");
    }
    out.push('+');
    out.push_str(&"-".repeat(viewport_width));
    out.push_str("+\x1b[K\r\n");
    push_terminal_line(
        &mut out,
        "A/D thrust | SPACE jump | J/L steer | ENTER charge/fire | 1/2 weapon | Ctrl-C exit",
        terminal_width,
    );
    for index in 0..feed_lines {
        if let Some(item) = game.feed.get(index) {
            push_terminal_line(&mut out, &format!("> {item}"), terminal_width);
        } else {
            push_terminal_line(&mut out, "", terminal_width);
        }
    }
    out.push_str("\x1b[J");
    out
}

pub fn incremental_frame(previous: Option<&str>, current: &str) -> String {
    let Some(previous) = previous else {
        return current.to_owned();
    };
    let Some(previous_rows) = frame_rows(previous) else {
        return current.to_owned();
    };
    let Some(current_rows) = frame_rows(current) else {
        return current.to_owned();
    };
    if previous_rows.len() != current_rows.len() {
        return current.to_owned();
    }

    let mut update = String::new();
    for (row, (old, new)) in previous_rows.iter().zip(&current_rows).enumerate() {
        if old != new {
            let _ = write!(update, "\x1b[{};1H{new}", row + 1);
        }
    }
    update
}

fn frame_rows(frame: &str) -> Option<Vec<&str>> {
    let body = frame.strip_prefix("\x1b[H")?.strip_suffix("\x1b[J")?;
    let body = body.strip_suffix("\r\n").unwrap_or(body);
    Some(body.split("\r\n").collect())
}

fn sampled_terrain(game: &Game, sx: usize, sy: usize, width: usize, height: usize) -> char {
    let x0 = sx * WIDTH / width;
    let x1 = ((sx + 1) * WIDTH).div_ceil(width).min(WIDTH);
    let y0 = sy * HEIGHT / height;
    let y1 = ((sy + 1) * HEIGHT).div_ceil(height).min(HEIGHT);
    if x1 <= x0 || y1 <= y0 {
        return ' ';
    }
    let xm = (x0 + x1) / 2;
    let ym = (y0 + y1) / 2;
    let tl = quadrant_filled(game, x0, xm.max(x0 + 1), y0, ym.max(y0 + 1));
    let tr = quadrant_filled(game, xm, x1, y0, ym.max(y0 + 1));
    let bl = quadrant_filled(game, x0, xm.max(x0 + 1), ym, y1);
    let br = quadrant_filled(game, xm, x1, ym, y1);
    quadrant_char(tl, tr, bl, br)
}

fn quadrant_filled(game: &Game, x0: usize, x1: usize, y0: usize, y1: usize) -> bool {
    if x1 <= x0 || y1 <= y0 {
        return false;
    }
    let total = (x1 - x0) * (y1 - y0);
    let mut earth = 0usize;
    for y in y0..y1 {
        for x in x0..x1 {
            if game.tile(x as i32, y as i32) == Tile::Earth {
                earth += 1;
            }
        }
    }
    earth * 2 >= total
}

fn quadrant_char(tl: bool, tr: bool, bl: bool, br: bool) -> char {
    match (tl, tr, bl, br) {
        (false, false, false, false) => ' ',
        (true, false, false, false) => '▘',
        (false, true, false, false) => '▝',
        (false, false, true, false) => '▖',
        (false, false, false, true) => '▗',
        (true, true, false, false) => '▀',
        (false, false, true, true) => '▄',
        (true, false, true, false) => '▌',
        (false, true, false, true) => '▐',
        (true, false, false, true) => '▚',
        (false, true, true, false) => '▞',
        (true, true, true, false) => '▛',
        (true, true, false, true) => '▜',
        (true, false, true, true) => '▙',
        (false, true, true, true) => '▟',
        (true, true, true, true) => '█',
    }
}

const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

fn level_split(value: i32) -> (i32, i32) {
    let bottom = value.min(8);
    let top = (value - 8).max(0);
    (bottom, top)
}

fn level_char(level: i32) -> char {
    if level <= 0 {
        ' '
    } else {
        BLOCKS[(level as usize - 1).min(7)]
    }
}

fn push_terminal_line(out: &mut String, text: &str, width: usize) {
    for ch in text.chars().take(width) {
        out.push(ch);
    }
    out.push_str("\x1b[K\r\n");
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Plain,
    Earth,
    Local,
    Enemy,
    Weapon,
    Aim,
    Health,
}

impl Style {
    fn escape(self, depth: ColorDepth) -> &'static str {
        match (depth, self) {
            (ColorDepth::Mono, _) | (_, Style::Plain) => "\x1b[0m",
            (ColorDepth::Ansi16, Style::Earth) => "\x1b[33m",
            (ColorDepth::Ansi16, Style::Local) => "\x1b[1;36m",
            (ColorDepth::Ansi16, Style::Enemy) => "\x1b[1;35m",
            (ColorDepth::Ansi16, Style::Weapon) => "\x1b[1;31m",
            (ColorDepth::Ansi16, Style::Aim) => "\x1b[1;37m",
            (ColorDepth::Ansi16, Style::Health) => "\x1b[1;32m",
            (ColorDepth::Ansi256, Style::Earth) => "\x1b[38;5;130m",
            (ColorDepth::Ansi256, Style::Local) => "\x1b[1;38;5;51m",
            (ColorDepth::Ansi256, Style::Enemy) => "\x1b[1;38;5;213m",
            (ColorDepth::Ansi256, Style::Weapon) => "\x1b[1;38;5;203m",
            (ColorDepth::Ansi256, Style::Aim) => "\x1b[1;38;5;231m",
            (ColorDepth::Ansi256, Style::Health) => "\x1b[1;38;5;82m",
            (ColorDepth::TrueColor, Style::Earth) => "\x1b[38;2;166;105;58m",
            (ColorDepth::TrueColor, Style::Local) => "\x1b[1;38;2;0;238;255m",
            (ColorDepth::TrueColor, Style::Enemy) => "\x1b[1;38;2;255;105;210m",
            (ColorDepth::TrueColor, Style::Weapon) => "\x1b[1;38;2;255;72;54m",
            (ColorDepth::TrueColor, Style::Aim) => "\x1b[1;38;2;255;245;190m",
            (ColorDepth::TrueColor, Style::Health) => "\x1b[1;38;2;110;255;110m",
        }
    }
}

fn style_for(ch: char, glyphs: GlyphMode) -> Style {
    match (glyphs, ch) {
        (GlyphMode::Ascii, '@') => Style::Local,
        (GlyphMode::Ascii, 'w') => Style::Enemy,
        (GlyphMode::Ascii, '*' | 'o' | '=') => Style::Weapon,
        (GlyphMode::Ascii, '+') => Style::Aim,
        (_, '|') => Style::Health,
        (GlyphMode::Powerlevel10k, '\u{f188}') => Style::Local,
        (GlyphMode::Powerlevel10k, '\u{f2db}') => Style::Enemy,
        (GlyphMode::Powerlevel10k, '\u{f135}' | '\u{f0e7}') => Style::Weapon,
        (GlyphMode::Powerlevel10k, '\u{f140}') => Style::Aim,
        (_, '#' | '%' | ':' | '.') => Style::Earth,
        (_, '▁' | '▂' | '▃' | '▄' | '▅' | '▆' | '▇' | '█') => Style::Earth,
        (_, '▀' | '▌' | '▐' | '▖' | '▗' | '▘' | '▝' | '▚' | '▞') => Style::Earth,
        (_, '▙' | '▟' | '▛' | '▜') => Style::Earth,
        (_, '▰') => Style::Weapon,
        (_, '◆') => Style::Health,
        _ => Style::Plain,
    }
}

fn plot_world(canvas: &mut [Vec<char>], x: i32, y: i32, ch: char) {
    if x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
        return;
    }
    let width = canvas.first().map_or(0, Vec::len);
    let height = canvas.len();
    let sx = x as usize * width / WIDTH;
    let sy = y as usize * height / HEIGHT;
    if let Some(row) = canvas.get_mut(sy)
        && let Some(cell) = row.get_mut(sx)
    {
        *cell = ch;
    }
}

fn plot_canvas_styled(
    canvas: &mut [Vec<char>],
    overlay: &mut [Vec<Option<Style>>],
    cx: i32,
    cy: i32,
    ch: char,
    style: Style,
) {
    if cx < 0 || cy < 0 {
        return;
    }
    let (sx, sy) = (cx as usize, cy as usize);
    if let Some(row) = canvas.get_mut(sy)
        && let Some(cell) = row.get_mut(sx)
    {
        *cell = ch;
    }
    if let Some(row) = overlay.get_mut(sy)
        && let Some(cell) = row.get_mut(sx)
    {
        *cell = Some(style);
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;
    use crate::game::Event;

    fn join(game: &mut Game, id: PlayerId, username: &str) {
        let (frames, _rx) = mpsc::unbounded_channel();
        game.accept(Event::Join {
            id,
            username: username.into(),
            frames,
            columns: 102,
            rows: 41,
            colors: ColorDepth::Mono,
            glyphs: GlyphMode::Ascii,
        });
    }

    #[test]
    fn frame_identifies_local_worm_and_controls() {
        let mut game = Game::new(1);
        join(&mut game, 4, "shell_user");

        let rendered = frame(&game, 4, 102, 41, ColorDepth::Mono, GlyphMode::Ascii);

        assert!(rendered.contains("shell_user"));
        assert!(rendered.contains("SPACE jump"));
        assert!(rendered.contains('@'));
        assert!(rendered.contains("AIM:"));
    }

    #[test]
    fn compact_terminal_resamples_the_complete_shared_battlefield() {
        let mut game = Game::new(2);
        join(&mut game, 1, "west");
        join(&mut game, 2, "east");
        game.players.get_mut(&1).expect("west exists").x = 1.0;
        game.players.get_mut(&2).expect("east exists").x = 98.0;

        let rendered = frame(&game, 1, 32, 18, ColorDepth::Mono, GlyphMode::Ascii);

        assert!(rendered.contains('@'));
        assert!(rendered.contains('w'));
        assert!(rendered.contains("+-----------------------------+"));
    }

    #[test]
    fn truecolor_powerlevel10k_client_receives_color_and_icon_glyphs() {
        let mut game = Game::new(3);
        join(&mut game, 1, "iconic");

        let rendered = frame(
            &game,
            1,
            102,
            41,
            ColorDepth::TrueColor,
            GlyphMode::Powerlevel10k,
        );

        assert!(rendered.contains("\x1b[1;38;2;0;238;255m"));
        assert!(rendered.contains('\u{f188}'));
    }

    #[test]
    fn animation_frames_do_not_leave_the_client_cursor_hidden() {
        let mut game = Game::new(4);
        join(&mut game, 1, "terminal");

        let rendered = frame(&game, 1, 102, 41, ColorDepth::Ansi16, GlyphMode::Ascii);

        assert!(
            !rendered.contains("\x1b[?25l"),
            "an abruptly closed SSH channel cannot repair a hidden cursor"
        );
    }

    #[test]
    fn large_terminal_uses_additional_columns_and_rows_for_the_battlefield() {
        let mut game = Game::new(5);
        join(&mut game, 1, "wide");

        let rendered = frame(&game, 1, 152, 64, ColorDepth::Mono, GlyphMode::Ascii);

        assert!(
            rendered.contains(&format!("+{}+", "-".repeat(149))),
            "large terminal columns must extend the rendered battlefield while reserving the wrap column"
        );
        assert!(
            rendered.lines().count() >= 64,
            "large terminal rows must extend the rendered battlefield"
        );
        assert!(
            rendered.contains('.'),
            "upscaled terrain must retain shaded ASCII surface detail"
        );
    }

    #[test]
    fn frame_never_draws_into_the_terminal_autowrap_column() {
        let mut game = Game::new(6);
        join(&mut game, 1, "steady");

        let rendered = frame(&game, 1, 80, 40, ColorDepth::Mono, GlyphMode::Ascii);

        assert!(rendered.contains(&format!("+{}+", "-".repeat(77))));
        assert!(
            !rendered.contains(&format!("+{}+", "-".repeat(78))),
            "writing the rightmost terminal column can cause the next frame to jump"
        );
    }

    #[test]
    fn incremental_render_only_sends_changed_rows_for_a_stable_viewport() {
        let mut game = Game::new(7);
        join(&mut game, 1, "efficient");
        let before = frame(&game, 1, 102, 41, ColorDepth::Mono, GlyphMode::Ascii);
        game.tick_number += 1;
        let after = frame(&game, 1, 102, 41, ColorDepth::Mono, GlyphMode::Ascii);

        let update = incremental_frame(Some(&before), &after);

        assert!(
            update.is_empty(),
            "a simulation tick with no visible state change must send no terminal rows"
        );
    }

    #[test]
    fn another_player_joining_updates_rows_without_forcing_a_full_frame() {
        let mut game = Game::new(8);
        join(&mut game, 1, "viewer");
        let before = frame(&game, 1, 80, 24, ColorDepth::Mono, GlyphMode::Ascii);
        join(&mut game, 2, "arrival");
        let after = frame(&game, 1, 80, 24, ColorDepth::Mono, GlyphMode::Ascii);

        let update = incremental_frame(Some(&before), &after);

        assert!(update.contains("P:2"));
        assert!(update.contains("arrival tunneled into the arena"));
        assert!(
            !update.contains("\x1b[H"),
            "a remote player event should remain a row delta"
        );
    }
}
