use std::collections::{HashMap, VecDeque};

use rand::{Rng, SeedableRng, rngs::SmallRng};
use tokio::sync::mpsc::UnboundedSender;

use crate::render;

pub const NATIVE_WIDTH: usize = 77;
pub const NATIVE_HEIGHT: usize = 18;
pub const WORLD_SCALE: usize = 4;
pub const WIDTH: usize = NATIVE_WIDTH * WORLD_SCALE;
pub const HEIGHT: usize = NATIVE_HEIGHT * WORLD_SCALE;
pub const TICK_RATE: u64 = 50;
const REF_RATE: f32 = 20.0;
const V_SCALE: f32 = REF_RATE / TICK_RATE as f32;
const A_SCALE: f32 = V_SCALE * V_SCALE;
const SCALE: f32 = WORLD_SCALE as f32;
const GRAVITY: f32 = 0.11 * SCALE * A_SCALE / Y_ASPECT;
const MAX_FALL: f32 = 1.45 * SCALE * V_SCALE / Y_ASPECT;
const MOVE_ACCEL: f32 = 0.18 * SCALE * A_SCALE;
const MAX_RUN: f32 = 0.65 * SCALE * V_SCALE;
const FRICTION: f32 = 0.876;
const SLIDE_ACCEL: f32 = 0.12 * SCALE * A_SCALE;
const SLIDE_LOOKAHEAD: i32 = 2;
const SLIDE_DROP_THRESHOLD: i32 = 4;
const FIRE_RELEASE_SILENCE: u64 = TICK_RATE / 4;
pub const MAX_CHARGE_TICKS: u64 = TICK_RATE * 2;
pub const POWER_STEP_TICKS: u64 = TICK_RATE / 10;
pub const POWER_STEP_PERCENT: u32 = 5;
const MAX_PROJECTILE_SPEED: f32 = 6.4 * V_SCALE;
const TERRAIN_CAP: usize = WIDTH * HEIGHT * 3 / 5;
const METEOR_INTERVAL_TICKS: u64 = TICK_RATE * 20;
const METEOR_OWNER: PlayerId = u64::MAX;
const MOVE_SUBSTEP: f32 = 0.9;
const FRICTION_GRACE_TICKS: u64 = TICK_RATE * 3 / 20;
const HANG_FALL_DELAY: u16 = TICK_RATE as u16;
const TALUS: i32 = 2;
const SLIDE_BASE_PROBABILITY: f64 = 0.55;
const GROWTH_HEIGHT_CAP_PCT: i32 = 70;
const GROWTH_MIN_BURST: usize = 6;
const GROWTH_MAX_BURST: usize = 60;
pub const MAX_HEALTH: i16 = 1000;
const HEALTH_REGEN_TICKS: u64 = TICK_RATE * 20;
pub const Y_ASPECT: f32 = 1.7;
const AIM_COOLDOWN_TICKS: u64 = TICK_RATE / 8;
const FEED_TTL_TICKS: u64 = TICK_RATE * 8;

const FRAG_VERBS: &[&str] = &[
    "vaporized",
    "deleted",
    "yeeted",
    "atomized",
    "obliterated",
    "rocket-tagged",
    "sent to the shadow realm",
    "introduced to gravity",
    "uninstalled",
    "tactically removed",
    "redistributed across the map",
    "converted to particle effects",
    "popped",
    "decommissioned",
    "kebab'd",
    "memory-freed",
    "404'd",
    "garbage-collected",
    "minced",
    "vented to space",
    "field-stripped",
    "force-pushed off the map",
    "discombobulated",
    "explained gravity to",
    "added to the changelog",
    "rebooted with prejudice",
    "applied percussive maintenance to",
    "evicted",
    "rage-quit on behalf of",
    "deprecated",
    "served a takedown notice to",
    "negotiated with a missile against",
];

const SUICIDE_VERBS: &[&str] = &[
    "cratered themselves",
    "found out gravity is real",
    "forgot they were holding it",
    "skipped the middleman",
    "rage-quit via grenade",
    "ate their own rocket",
    "self-recycled",
    "demonstrated the blast radius",
    "tested the weapon on themselves",
    "yeeted themselves into the void",
    "ran their own QA",
    "achieved enlightenment, painfully",
    "speedran the respawn timer",
    "panicked and pressed Enter",
    "DM'd themselves a rocket",
    "discovered the muzzle was reversed",
    "wrote their own obituary",
    "unsubscribed from the server",
    "took the express lane to respawn",
    "applied the patch directly to face",
    "auto-completed their own funeral",
    "selected the wrong target lock",
];

const MUTUAL_VERBS: &[&str] = &[
    "annihilated each other",
    "exchanged farewells via ordnance",
    "ran into each other's missiles",
    "agreed to disagree, explosively",
    "double-tapped each other",
    "shook hands and detonated",
    "demonstrated peer review",
    "high-fived through a grenade",
    "tied for worst tactical decision",
    "deleted each other from existence",
    "synchronized their respawn timers",
    "achieved mutually assured deletion",
];

const METEOR_VERBS: &[&str] = &[
    "got cosmically yeeted",
    "made a fossil of themselves",
    "lost an argument with a falling rock",
    "served as the dinosaurs' revenge target",
    "found out about gravity from space",
    "was selected by the heavens",
    "starred in their own extinction event",
    "received express delivery from the void",
];

const JOIN_VERBS: &[&str] = &[
    "tunneled into the arena",
    "wormed in",
    "joined the party uninvited",
    "spawned with intent",
    "is here to ruin somebody's day",
    "materialized from the soil",
    "logged in for chaos",
    "wriggled onto the scene",
    "appeared, looking suspicious",
    "got out of bed for this",
    "checked in for the apocalypse",
    "rolled the spawn dice",
    "answered the call",
    "showed up with a rocket and a dream",
];
const MIN_BLAST_DAMAGE: i16 = 40;

pub type PlayerId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    Mono,
    Ansi16,
    Ansi256,
    TrueColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphMode {
    Ascii,
    Powerlevel10k,
}

#[derive(Debug)]
pub enum Event {
    Join {
        id: PlayerId,
        username: String,
        frames: UnboundedSender<String>,
        columns: u32,
        rows: u32,
        colors: ColorDepth,
        glyphs: GlyphMode,
    },
    Leave(PlayerId),
    Input {
        id: PlayerId,
        input: Vec<u8>,
    },
    Resize {
        id: PlayerId,
        columns: u32,
        rows: u32,
    },
    Color {
        id: PlayerId,
        colors: ColorDepth,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tile {
    Air,
    Earth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weapon {
    Bazooka,
    Grenade,
    Meteor,
}

#[derive(Debug, Clone)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub health: i16,
    pub kills: u32,
    pub deaths: u32,
    pub aim: i8,
    pub facing: i8,
    pub weapon: Weapon,
    pub respawn_ticks: u16,
    pub vx: f32,
    vy: f32,
    fire_cooldown: u16,
    move_impulse: f32,
    jump: bool,
    pub charge_started: Option<u64>,
    last_charge_pulse: Option<u64>,
    last_move_tick: u64,
    movement_started: u64,
    last_direction: i8,
    last_input_tick: u64,
    regen_carry: i32,
    next_aim_tick: u64,
}

#[derive(Debug, Clone)]
pub struct Projectile {
    pub x: f32,
    pub y: f32,
    pub glyph: char,
    pub owner: PlayerId,
    weapon: Weapon,
    vx: f32,
    vy: f32,
    fuse: u16,
}

#[derive(Debug, Clone)]
pub struct Blast {
    pub x: i32,
    pub y: i32,
    pub radius: i32,
    pub age: u8,
}

struct Client {
    player: PlayerId,
    frames: UnboundedSender<String>,
    last_frame: Option<String>,
    columns: u32,
    rows: u32,
    colors: ColorDepth,
    glyphs: GlyphMode,
}

pub struct Game {
    pub tiles: Vec<Tile>,
    pub players: HashMap<PlayerId, Player>,
    pub projectiles: Vec<Projectile>,
    pub blasts: Vec<Blast>,
    pub tick_number: u64,
    pub feed: VecDeque<(u64, String)>,
    rng: SmallRng,
    clients: HashMap<PlayerId, Client>,
    next_growth_tick: u64,
    next_meteor_tick: u64,
    hang: Vec<u16>,
    cohesion: Vec<u8>,
    pending_kills: Vec<(PlayerId, PlayerId)>,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut game = Self {
            tiles: vec![Tile::Air; WIDTH * HEIGHT],
            players: HashMap::new(),
            projectiles: Vec::new(),
            blasts: Vec::new(),
            tick_number: 0,
            feed: VecDeque::new(),
            rng: SmallRng::seed_from_u64(seed),
            clients: HashMap::new(),
            next_growth_tick: 40,
            next_meteor_tick: METEOR_INTERVAL_TICKS,
            hang: vec![0; WIDTH * HEIGHT],
            cohesion: vec![0; WIDTH * HEIGHT],
            pending_kills: Vec::new(),
        };
        game.generate_world();
        game
    }

    pub fn accept(&mut self, event: Event) {
        match event {
            Event::Join {
                id,
                username,
                frames,
                columns,
                rows,
                colors,
                glyphs,
            } => {
                let name = self.available_name(&sanitize_name(&username), id);
                let (x, y) = self.spawn_point();
                self.players
                    .insert(id, Player::spawned(id, name.clone(), x, y));
                self.clients.insert(
                    id,
                    Client {
                        player: id,
                        frames,
                        last_frame: None,
                        columns,
                        rows,
                        colors,
                        glyphs,
                    },
                );
                let verb = JOIN_VERBS[self.rng.random_range(0..JOIN_VERBS.len())];
                self.say(format!("{name} {verb}"));
            }
            Event::Leave(id) => {
                self.clients.remove(&id);
                if let Some(player) = self.players.remove(&id) {
                    self.say(format!("{} vanished", player.name));
                }
            }
            Event::Input { id, input } => self.handle_input(id, &input),
            Event::Resize { id, columns, rows } => {
                if let Some(client) = self.clients.get_mut(&id) {
                    client.columns = columns;
                    client.rows = rows;
                }
            }
            Event::Color { id, colors } => {
                if let Some(client) = self.clients.get_mut(&id) {
                    client.colors = colors;
                }
            }
        }
    }

    pub fn tick(&mut self) {
        self.tick_number += 1;
        let cutoff = self.tick_number.saturating_sub(FEED_TTL_TICKS);
        self.feed.retain(|(t, _)| *t >= cutoff);
        self.update_players();
        self.update_projectiles();
        self.flush_kills();
        self.settle_terrain();
        if self.tick_number >= self.next_growth_tick {
            self.grow_terrain();
            let earth = self.earth_count();
            let fill = (earth as f32 / TERRAIN_CAP as f32).clamp(0.0, 1.0);
            let delay_sec = 3.0 + fill * fill * 25.0;
            let delay = (delay_sec * TICK_RATE as f32) as u64;
            self.next_growth_tick = self.tick_number + delay.max(TICK_RATE * 3);
        }
        if self.tick_number >= self.next_meteor_tick {
            self.spawn_meteor();
            self.next_meteor_tick = self.tick_number + METEOR_INTERVAL_TICKS;
        }
        self.blasts.retain_mut(|blast| {
            blast.age += 1;
            blast.age < 8
        });
    }

    pub fn broadcast(&mut self) {
        let frames: Vec<(PlayerId, String)> = self
            .clients
            .values()
            .map(|client| {
                (
                    client.player,
                    render::frame(
                        self,
                        client.player,
                        client.columns,
                        client.rows,
                        client.colors,
                        client.glyphs,
                    ),
                )
            })
            .collect();
        let mut stale = Vec::new();
        for (id, frame) in frames {
            let Some(client) = self.clients.get_mut(&id) else {
                continue;
            };
            let update = render::incremental_frame(client.last_frame.as_deref(), &frame);
            client.last_frame = Some(frame);
            if !update.is_empty() && client.frames.send(update).is_err() {
                stale.push(id);
            }
        }
        for id in stale {
            self.clients.remove(&id);
            self.players.remove(&id);
        }
    }

    pub fn tile(&self, x: i32, y: i32) -> Tile {
        if y < 0 {
            return Tile::Air;
        }
        if x < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
            return Tile::Earth;
        }
        self.tiles[y as usize * WIDTH + x as usize]
    }

    fn set_tile(&mut self, x: i32, y: i32, tile: Tile) {
        if x >= 0 && y >= 0 && x < WIDTH as i32 && y < HEIGHT as i32 {
            let idx = y as usize * WIDTH + x as usize;
            self.tiles[idx] = tile;
            if tile == Tile::Air {
                self.cohesion[idx] = 0;
            }
        }
    }

    fn seed_cohesion(&mut self, x: i32, y: i32) {
        if x >= 0 && y >= 0 && x < WIDTH as i32 && y < HEIGHT as i32 {
            let idx = y as usize * WIDTH + x as usize;
            self.cohesion[idx] = self.rng.random_range(1..=3);
        }
    }

    fn generate_world(&mut self) {
        let mut surface = (HEIGHT * 2 / 3) as i32;
        for x in 0..WIDTH as i32 {
            surface = (surface + self.rng.random_range(-1..=1))
                .clamp((HEIGHT / 2) as i32, (HEIGHT - 6) as i32);
            for y in surface..HEIGHT as i32 {
                self.set_tile(x, y, Tile::Earth);
                self.seed_cohesion(x, y);
            }
        }
        for _ in 0..10 {
            let x = self.rng.random_range(8..(WIDTH - 8)) as i32;
            let y = self.rng.random_range((HEIGHT / 2)..(HEIGHT - 3)) as i32;
            let radius = self.rng.random_range((2 * WORLD_SCALE)..=(4 * WORLD_SCALE)) as i32;
            self.carve(x, y, radius);
        }
    }

    fn available_name(&self, username: &str, id: PlayerId) -> String {
        if self.players.values().all(|player| player.name != username) {
            username.to_owned()
        } else {
            format!("{username}#{id}")
        }
    }

    fn spawn_point(&mut self) -> (f32, f32) {
        for _ in 0..100 {
            let x = self.rng.random_range(3..(WIDTH - 3)) as i32;
            for y in 2..(HEIGHT - 2) as i32 {
                if self.tile(x, y) == Tile::Air && self.tile(x, y + 1) == Tile::Earth {
                    let occupied = self.players.values().any(|p| (p.x - x as f32).abs() < 4.0);
                    if !occupied {
                        return (x as f32, y as f32);
                    }
                }
            }
        }
        (4.0, 2.0)
    }

    fn handle_input(&mut self, id: PlayerId, input: &[u8]) {
        let tick = self.tick_number;
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let mut bytes = input.iter().copied().peekable();
        while let Some(byte) = bytes.next() {
            if byte == 0x1b {
                let Some(intro) = bytes.next() else {
                    continue;
                };
                if intro != b'[' && intro != b'O' {
                    continue;
                }
                let mut terminator = None;
                for next in bytes.by_ref() {
                    if next.is_ascii_alphabetic() || next == b'~' {
                        terminator = Some(next);
                        break;
                    }
                }
                match terminator {
                    Some(b'A') if tick >= player.next_aim_tick => {
                        player.aim = (player.aim - 1).clamp(-8, 8);
                        player.next_aim_tick = tick + AIM_COOLDOWN_TICKS;
                    }
                    Some(b'B') if tick >= player.next_aim_tick => {
                        player.aim = (player.aim + 1).clamp(-8, 8);
                        player.next_aim_tick = tick + AIM_COOLDOWN_TICKS;
                    }
                    Some(b'C') => player.pulse_move(1, tick),
                    Some(b'D') => player.pulse_move(-1, tick),
                    _ => {}
                }
                continue;
            }
            match byte {
                b'a' | b'A' => player.pulse_move(-1, tick),
                b'd' | b'D' => player.pulse_move(1, tick),
                b' ' => player.jump = true,
                b'w' | b'W' | b'j' | b'J' if tick >= player.next_aim_tick => {
                    player.aim = (player.aim - 1).clamp(-8, 8);
                    player.next_aim_tick = tick + AIM_COOLDOWN_TICKS;
                }
                b's' | b'S' | b'l' | b'L' if tick >= player.next_aim_tick => {
                    player.aim = (player.aim + 1).clamp(-8, 8);
                    player.next_aim_tick = tick + AIM_COOLDOWN_TICKS;
                }
                b'1' => player.weapon = Weapon::Bazooka,
                b'2' => player.weapon = Weapon::Grenade,
                b'\r' | b'\n' | b'x' | b'X' | b'f' | b'F' => {
                    if player.charge_started.is_none() && player.fire_cooldown == 0 {
                        player.charge_started = Some(tick);
                    }
                    if player.charge_started.is_some() {
                        player.last_charge_pulse = Some(tick);
                    }
                }
                _ => {}
            }
        }
    }

    fn update_players(&mut self) {
        let ids: Vec<PlayerId> = self.players.keys().copied().collect();
        for id in ids {
            let needs_spawn = self
                .players
                .get(&id)
                .is_some_and(|player| player.respawn_ticks == 1);
            if needs_spawn {
                let (x, y) = self.spawn_point();
                if let Some(player) = self.players.get_mut(&id) {
                    player.reset_at(x, y);
                }
                continue;
            }

            let grounded = self.players.get(&id).is_some_and(|player| {
                self.tile(player.x.round() as i32, player.y.round() as i32 + 1) == Tile::Earth
            });
            let slide = self
                .players
                .get(&id)
                .filter(|_| grounded)
                .map_or(0.0, |player| self.slide_impulse(player.x.round() as i32));
            let mut projectile = None;
            if let Some(player) = self.players.get_mut(&id) {
                if player.respawn_ticks > 0 {
                    player.respawn_ticks -= 1;
                    continue;
                }
                player.fire_cooldown = player.fire_cooldown.saturating_sub(1);
                if player.health > 0 && player.health < MAX_HEALTH {
                    player.regen_carry += MAX_HEALTH as i32;
                    let step = (player.regen_carry / HEALTH_REGEN_TICKS as i32) as i16;
                    if step > 0 {
                        player.health = (player.health + step).min(MAX_HEALTH);
                        player.regen_carry -= step as i32 * HEALTH_REGEN_TICKS as i32;
                    }
                }
                player.vx = (player.vx + player.move_impulse + slide).clamp(-MAX_RUN, MAX_RUN);
                let idle =
                    self.tick_number.saturating_sub(player.last_input_tick) > FRICTION_GRACE_TICKS;
                if player.move_impulse == 0.0 && idle {
                    player.vx *= FRICTION;
                }
                if player.jump && grounded {
                    player.vy = (-(0.7 * SCALE) - player.vx.abs() * 0.3) * V_SCALE / Y_ASPECT;
                }
                player.vy = (player.vy + GRAVITY).min(MAX_FALL);
                if let Some(start) = player.charge_started
                    && (self.tick_number.saturating_sub(start) >= MAX_CHARGE_TICKS
                        || player.last_charge_pulse.is_some_and(|pulse| {
                            self.tick_number.saturating_sub(pulse) >= FIRE_RELEASE_SILENCE
                        }))
                    && player.fire_cooldown == 0
                {
                    let elapsed = self.tick_number.saturating_sub(start).min(MAX_CHARGE_TICKS);
                    let power_pct =
                        ((elapsed / POWER_STEP_TICKS) as u32 * POWER_STEP_PERCENT).min(100);
                    projectile = Some(player.fire(power_pct));
                    player.charge_started = None;
                    player.last_charge_pulse = None;
                }
                player.move_impulse = 0.0;
                player.jump = false;
            }
            self.move_player(id);
            if let Some(projectile) = projectile {
                self.projectiles.push(projectile);
            }
        }
    }

    fn move_player(&mut self, id: PlayerId) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let (mut x, mut y, vx, vy) = (player.x, player.y, player.vx, player.vy);
        let steps = (vx.abs().max(vy.abs()) / MOVE_SUBSTEP).ceil().max(1.0) as i32;
        let dx = vx / steps as f32;
        let dy = vy / steps as f32;
        let mut blocked_y = false;
        for _ in 0..steps {
            let nx = x + dx;
            let nxi = nx.round() as i32;
            let yi = y.round() as i32;
            if self.tile(nxi, yi) == Tile::Air {
                x = nx;
            } else if self.tile(nxi, yi - 1) == Tile::Air && self.tile(nxi, yi - 2) == Tile::Air {
                x = nx;
                y -= 1.0;
            }
            let ny = y + dy;
            if self.tile(x.round() as i32, ny.round() as i32) == Tile::Air {
                y = ny;
            } else {
                blocked_y = true;
                break;
            }
        }
        if let Some(player) = self.players.get_mut(&id) {
            player.x = x;
            player.y = y;
            if blocked_y {
                player.vy = 0.0;
            }
        }
    }

    fn slide_impulse(&self, x: i32) -> f32 {
        let here = self.surface_height(x);
        let left = self.surface_height(x - SLIDE_LOOKAHEAD);
        let right = self.surface_height(x + SLIDE_LOOKAHEAD);
        match (here, left, right) {
            (Some(here), _, Some(right)) if right - here >= SLIDE_DROP_THRESHOLD => SLIDE_ACCEL,
            (Some(here), Some(left), _) if left - here >= SLIDE_DROP_THRESHOLD => -SLIDE_ACCEL,
            _ => 0.0,
        }
    }

    fn surface_height(&self, x: i32) -> Option<i32> {
        (0..HEIGHT as i32).find(|&y| self.tile(x, y) == Tile::Earth)
    }

    fn update_projectiles(&mut self) {
        let mut remaining = Vec::with_capacity(self.projectiles.len());
        let mut explosions = Vec::new();
        for mut projectile in std::mem::take(&mut self.projectiles) {
            projectile.vy += GRAVITY * 0.65;
            projectile.fuse = projectile.fuse.saturating_sub(1);
            let steps = (projectile.vx.abs().max(projectile.vy.abs()) / MOVE_SUBSTEP)
                .ceil()
                .max(1.0) as i32;
            let dx = projectile.vx / steps as f32;
            let dy = projectile.vy / steps as f32;
            let mut hit = false;
            for _ in 0..steps {
                projectile.x += dx;
                projectile.y += dy;
                if self.tile(projectile.x.round() as i32, projectile.y.round() as i32)
                    == Tile::Earth
                {
                    hit = true;
                    break;
                }
            }
            if hit || projectile.fuse == 0 {
                explosions.push((
                    projectile.x,
                    projectile.y,
                    projectile.owner,
                    projectile.weapon,
                ));
            } else {
                remaining.push(projectile);
            }
        }
        self.projectiles = remaining;
        for (x, y, owner, weapon) in explosions {
            let (radius, max_dmg) = match weapon {
                Weapon::Bazooka => (4 * WORLD_SCALE as i32, 800.0),
                Weapon::Grenade => (8 * WORLD_SCALE as i32, 1200.0),
                Weapon::Meteor => (12 * WORLD_SCALE as i32, 1500.0),
            };
            self.explode(x.round() as i32, y.round() as i32, radius, max_dmg, owner);
        }
    }

    fn explode(&mut self, x: i32, y: i32, radius: i32, max_dmg: f32, owner: PlayerId) {
        self.carve(x, y, radius);
        self.blasts.push(Blast {
            x,
            y,
            radius,
            age: 0,
        });
        let mut announcements = Vec::new();
        for player in self.players.values_mut() {
            if player.respawn_ticks > 0 {
                continue;
            }
            let dx = player.x - x as f32;
            let dy = player.y - y as f32;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > radius as f32 + 1.5 {
                continue;
            }
            let scale = max_dmg / (radius as f32 + 2.0);
            let damage = ((radius as f32 + 2.0 - distance) * scale)
                .clamp(MIN_BLAST_DAMAGE as f32, max_dmg) as i16;
            player.health -= damage;
            player.vx += dx.signum() * 0.8 * SCALE * V_SCALE;
            player.vy = -0.7 * SCALE * V_SCALE / Y_ASPECT;
            if player.health <= 0 {
                player.deaths += 1;
                player.respawn_ticks = (TICK_RATE * 3) as u16;
                announcements.push((player.id, player.name.clone()));
            }
        }
        for (victim, _) in announcements {
            if victim != owner
                && let Some(killer) = self.players.get_mut(&owner)
            {
                killer.kills += 1;
            }
            self.pending_kills.push((owner, victim));
        }
    }

    fn flush_kills(&mut self) {
        let kills = std::mem::take(&mut self.pending_kills);
        let mut consumed = vec![false; kills.len()];
        for i in 0..kills.len() {
            if consumed[i] {
                continue;
            }
            consumed[i] = true;
            let (k1, v1) = kills[i];
            let mut mutual = None;
            if k1 != v1 {
                for (j, &(k2, v2)) in kills.iter().enumerate().skip(i + 1) {
                    if !consumed[j] && k2 == v1 && v2 == k1 {
                        consumed[j] = true;
                        mutual = Some(());
                        break;
                    }
                }
            }
            let v1_name = self
                .players
                .get(&v1)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            if k1 == METEOR_OWNER {
                let verb = METEOR_VERBS[self.rng.random_range(0..METEOR_VERBS.len())];
                self.say(format!("{v1_name} {verb}"));
            } else if mutual.is_some() {
                let k1_name = self
                    .players
                    .get(&k1)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                let verb = MUTUAL_VERBS[self.rng.random_range(0..MUTUAL_VERBS.len())];
                self.say(format!("{k1_name} and {v1_name} {verb}"));
            } else if k1 == v1 {
                let verb = SUICIDE_VERBS[self.rng.random_range(0..SUICIDE_VERBS.len())];
                self.say(format!("{v1_name} {verb}"));
            } else {
                let k1_name = self
                    .players
                    .get(&k1)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                let verb = FRAG_VERBS[self.rng.random_range(0..FRAG_VERBS.len())];
                self.say(format!("{k1_name} {verb} {v1_name}"));
            }
        }
    }

    fn carve(&mut self, cx: i32, cy: i32, radius: i32) {
        for y in (cy - radius)..=(cy + radius) {
            for x in (cx - radius)..=(cx + radius) {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy <= radius * radius {
                    self.set_tile(x, y, Tile::Air);
                }
            }
        }
    }

    fn spawn_meteor(&mut self) {
        let x = self.rng.random_range(20..(WIDTH as i32 - 20)) as f32;
        let vx = self.rng.random_range(-0.4..=0.4);
        let vy = self.rng.random_range(2.5..=4.0);
        self.projectiles.push(Projectile {
            x,
            y: -SCALE,
            glyph: '@',
            owner: METEOR_OWNER,
            weapon: Weapon::Meteor,
            vx,
            vy,
            fuse: TICK_RATE as u16 * 6,
        });
        self.say("a meteor screams down from the sky".into());
    }

    fn grow_terrain(&mut self) {
        let earth = self.earth_count();
        if earth >= TERRAIN_CAP {
            return;
        }
        let deficit = (TERRAIN_CAP - earth) as f32 / TERRAIN_CAP as f32;
        let burst = ((deficit * GROWTH_MAX_BURST as f32) as usize)
            .clamp(GROWTH_MIN_BURST, GROWTH_MAX_BURST)
            .min(TERRAIN_CAP - earth);

        let max_top_y = HEIGHT as i32 * (100 - GROWTH_HEIGHT_CAP_PCT) / 100;
        let mut candidates: Vec<i32> = Vec::new();
        for x in 1..(WIDTH as i32 - 1) {
            let Some((_, top)) = self.supported_growth_site(x) else {
                continue;
            };
            if top >= max_top_y {
                candidates.push(x);
            }
        }
        if candidates.is_empty() {
            return;
        }

        let mut placed = 0;
        let mut attempts = 0;
        while placed < burst && !candidates.is_empty() && attempts < burst * 12 {
            attempts += 1;
            let idx = self.rng.random_range(0..candidates.len());
            let x = candidates[idx];
            let Some((_, top)) = self.supported_growth_site(x) else {
                candidates.swap_remove(idx);
                continue;
            };
            if top < max_top_y {
                candidates.swap_remove(idx);
                continue;
            }
            let max_push = (3.min(burst - placed)).max(1);
            let push = self.rng.random_range(1..=max_push) as i32;
            for k in 0..push {
                let y = top - k;
                if y < 0 {
                    break;
                }
                self.set_tile(x, y, Tile::Earth);
                self.seed_cohesion(x, y);
                self.hang[y as usize * WIDTH + x as usize] = 0;
                placed += 1;
            }
            self.lift_buried_players();
        }
    }

    fn supported_growth_site(&self, x: i32) -> Option<(i32, i32)> {
        let mut y = HEIGHT as i32 - 1;
        while y >= 0 && self.tile(x, y) == Tile::Earth {
            y -= 1;
        }
        (y >= 0).then_some((x, y))
    }

    fn settle_terrain(&mut self) {
        let mut anchored = vec![false; WIDTH * HEIGHT];
        let mut queue: VecDeque<(i32, i32)> = VecDeque::new();
        let bottom = HEIGHT as i32 - 1;
        for x in 0..WIDTH as i32 {
            if self.tile(x, bottom) == Tile::Earth {
                let idx = bottom as usize * WIDTH + x as usize;
                anchored[idx] = true;
                queue.push_back((x, bottom));
            }
        }
        while let Some((x, y)) = queue.pop_front() {
            for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                if nx < 0 || ny < 0 || nx >= WIDTH as i32 || ny >= HEIGHT as i32 {
                    continue;
                }
                let idx = ny as usize * WIDTH + nx as usize;
                if !anchored[idx] && self.tile(nx, ny) == Tile::Earth {
                    anchored[idx] = true;
                    queue.push_back((nx, ny));
                }
            }
        }

        let mut to_fall: Vec<(i32, i32)> = Vec::new();
        for y in 0..HEIGHT as i32 {
            for x in 0..WIDTH as i32 {
                if self.tile(x, y) != Tile::Earth {
                    continue;
                }
                let idx = y as usize * WIDTH + x as usize;
                if anchored[idx] {
                    self.hang[idx] = 0;
                } else {
                    self.hang[idx] = self.hang[idx].saturating_add(1);
                    if self.hang[idx] >= HANG_FALL_DELAY {
                        to_fall.push((x, y));
                    }
                }
            }
        }

        to_fall.sort_by_key(|&(_, y)| std::cmp::Reverse(y));
        for (x, y) in to_fall {
            if self.tile(x, y) != Tile::Earth {
                continue;
            }
            let mut ny = y;
            while ny + 1 < HEIGHT as i32 && self.tile(x, ny + 1) == Tile::Air {
                ny += 1;
            }
            if ny != y {
                let prev_cohesion = self.cohesion[y as usize * WIDTH + x as usize].max(1);
                self.set_tile(x, y, Tile::Air);
                self.set_tile(x, ny, Tile::Earth);
                let dst = ny as usize * WIDTH + x as usize;
                self.cohesion[dst] = prev_cohesion;
                self.hang[dst] = 0;
            }
        }

        self.slide_terrain();
        self.lift_buried_players();
    }

    fn slide_terrain(&mut self) {
        let reverse = self.rng.random_bool(0.5);
        let xs: Vec<i32> = if reverse {
            (0..WIDTH as i32).rev().collect()
        } else {
            (0..WIDTH as i32).collect()
        };
        for x in xs {
            let Some(top) = self.surface_height(x) else {
                continue;
            };
            if top >= HEIGHT as i32 - 1 {
                continue;
            }
            if self.tile(x, top + 1) != Tile::Earth {
                continue;
            }
            let left = self.surface_height(x - 1).unwrap_or(HEIGHT as i32);
            let right = self.surface_height(x + 1).unwrap_or(HEIGHT as i32);
            let left_diff = left - top;
            let right_diff = right - top;
            if left_diff < TALUS && right_diff < TALUS {
                continue;
            }
            let cohesion = self.cohesion[top as usize * WIDTH + x as usize].max(1) as f64;
            let slide_p = (SLIDE_BASE_PROBABILITY / cohesion).clamp(0.05, 0.9);
            if !self.rng.random_bool(slide_p) {
                continue;
            }
            let go_left = match left_diff.cmp(&right_diff) {
                std::cmp::Ordering::Greater => true,
                std::cmp::Ordering::Less => false,
                std::cmp::Ordering::Equal => self.rng.random_bool(0.5),
            };
            let nx = if go_left { x - 1 } else { x + 1 };
            if nx < 0 || nx >= WIDTH as i32 {
                continue;
            }
            let target_y = self
                .surface_height(nx)
                .map(|y| y - 1)
                .unwrap_or(HEIGHT as i32 - 1);
            if target_y < 0 || self.tile(nx, target_y) != Tile::Air {
                continue;
            }
            let prev_cohesion = self.cohesion[top as usize * WIDTH + x as usize];
            self.set_tile(x, top, Tile::Air);
            self.set_tile(nx, target_y, Tile::Earth);
            self.cohesion[target_y as usize * WIDTH + nx as usize] = prev_cohesion.max(1);
            self.hang[top as usize * WIDTH + x as usize] = 0;
            self.hang[target_y as usize * WIDTH + nx as usize] = 0;
        }
    }

    fn lift_buried_players(&mut self) {
        let mut buried: Vec<(PlayerId, String)> = Vec::new();
        let lifts: Vec<(PlayerId, f32)> = self
            .players
            .values()
            .filter(|player| {
                player.respawn_ticks == 0
                    && self.tile(player.x.round() as i32, player.y.round() as i32) == Tile::Earth
            })
            .map(|player| {
                let mut y = player.y;
                while y >= 0.0
                    && self.tile(player.x.round() as i32, y.round() as i32) == Tile::Earth
                {
                    y -= 1.0;
                }
                (player.id, y)
            })
            .collect();
        for (id, y) in lifts {
            if y < 0.0 {
                if let Some(player) = self.players.get_mut(&id) {
                    player.health = 0;
                    player.deaths += 1;
                    player.respawn_ticks = (TICK_RATE * 3) as u16;
                    buried.push((player.id, player.name.clone()));
                }
            } else if let Some(player) = self.players.get_mut(&id) {
                player.y = y;
                player.vy = player.vy.min(-0.25 * SCALE * V_SCALE / Y_ASPECT);
            }
        }
        for (_, name) in buried {
            self.say(format!("{name} was buried alive"));
        }
    }

    fn earth_count(&self) -> usize {
        self.tiles
            .iter()
            .filter(|tile| **tile == Tile::Earth)
            .count()
    }

    fn say(&mut self, message: String) {
        self.feed.push_front((self.tick_number, message));
        self.feed.truncate(4);
    }
}

impl Player {
    fn spawned(id: PlayerId, name: String, x: f32, y: f32) -> Self {
        Self {
            id,
            name,
            x,
            y,
            health: MAX_HEALTH,
            kills: 0,
            deaths: 0,
            aim: -1,
            facing: 1,
            weapon: Weapon::Bazooka,
            respawn_ticks: 0,
            vx: 0.0,
            vy: 0.0,
            fire_cooldown: 0,
            move_impulse: 0.0,
            jump: false,
            charge_started: None,
            last_charge_pulse: None,
            last_move_tick: 0,
            movement_started: 0,
            last_direction: 0,
            last_input_tick: 0,
            regen_carry: 0,
            next_aim_tick: 0,
        }
    }

    fn reset_at(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
        self.health = MAX_HEALTH;
        self.respawn_ticks = 0;
        self.vx = 0.0;
        self.vy = 0.0;
        self.regen_carry = 0;
        self.charge_started = None;
        self.last_charge_pulse = None;
        self.fire_cooldown = 0;
        self.move_impulse = 0.0;
        self.jump = false;
    }

    fn pulse_move(&mut self, direction: i8, tick: u64) {
        let gap = tick.saturating_sub(self.last_move_tick);
        if direction == self.last_direction && self.last_move_tick == tick {
            return;
        }
        if direction != self.last_direction || gap > 4 {
            self.movement_started = tick;
        }
        let held_ticks = tick.saturating_sub(self.movement_started).min(24);
        let acceleration = if gap <= 4 {
            MOVE_ACCEL + held_ticks as f32 * 0.012 * SCALE * A_SCALE
        } else {
            MOVE_ACCEL
        };
        self.move_impulse += direction as f32 * acceleration;
        self.facing = direction;
        self.last_direction = direction;
        self.last_move_tick = tick;
        self.last_input_tick = tick;
    }

    fn fire(&mut self, power_pct: u32) -> Projectile {
        self.fire_cooldown = match self.weapon {
            Weapon::Bazooka => 12,
            Weapon::Grenade => 20,
            Weapon::Meteor => 0,
        };
        let weapon_factor = match self.weapon {
            Weapon::Bazooka => 1.0,
            Weapon::Grenade => 0.45,
            Weapon::Meteor => 1.0,
        };
        let speed = MAX_PROJECTILE_SPEED * power_pct as f32 / 100.0 * weapon_factor;
        let angle = self.aim as f32 * std::f32::consts::PI / 16.0;
        let cos = angle.cos();
        let sin = angle.sin();
        Projectile {
            x: self.x + self.facing as f32 * SCALE,
            y: self.y - 0.3 * SCALE,
            glyph: match self.weapon {
                Weapon::Bazooka => '*',
                Weapon::Grenade => 'o',
                Weapon::Meteor => '@',
            },
            owner: self.id,
            weapon: self.weapon,
            vx: self.facing as f32 * speed * cos,
            vy: speed * sin,
            fuse: match self.weapon {
                Weapon::Bazooka => 200,
                Weapon::Grenade => 240,
                Weapon::Meteor => 200,
            },
        }
    }
}

fn sanitize_name(input: &str) -> String {
    let clean: String = input
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
        .take(18)
        .collect();
    if clean.is_empty() {
        "worm".to_owned()
    } else {
        clean
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;

    fn join(game: &mut Game, id: PlayerId, username: &str) {
        let (frames, _rx) = mpsc::unbounded_channel();
        game.accept(Event::Join {
            id,
            username: username.to_owned(),
            frames,
            columns: WIDTH as u32 + 2,
            rows: HEIGHT as u32 + 7,
            colors: ColorDepth::Mono,
            glyphs: GlyphMode::Ascii,
        });
    }

    #[test]
    fn duplicate_ssh_usernames_keep_distinct_live_worms() {
        let mut game = Game::new(8);
        join(&mut game, 1, "alice");
        join(&mut game, 2, "alice");

        assert_eq!(game.players[&1].name, "alice");
        assert_eq!(game.players[&2].name, "alice#2");
    }

    #[test]
    fn explosion_carves_terrain_and_damages_nearby_worm() {
        let mut game = Game::new(9);
        join(&mut game, 1, "shooter");
        join(&mut game, 2, "target");
        let target = game.players.get_mut(&2).expect("target exists");
        target.x = 30.0;
        target.y = 12.0;
        game.set_tile(30, 12, Tile::Earth);

        game.explode(30, 12, 4, 800.0, 1);

        assert_eq!(game.tile(30, 12), Tile::Air);
        assert!(game.players[&2].health < MAX_HEALTH);
    }

    #[test]
    fn dead_worm_respawns_with_full_health_after_delay() {
        let mut game = Game::new(10);
        join(&mut game, 1, "target");
        let target = game.players.get_mut(&1).expect("target exists");
        target.x = 20.0;
        target.y = 10.0;
        target.health = 1;
        game.explode(20, 10, 5, 800.0, 1);
        assert!(game.players[&1].respawn_ticks > 0);

        for _ in 0..=(TICK_RATE * 3) {
            game.tick();
        }

        assert_eq!(game.players[&1].health, MAX_HEALTH);
        assert_eq!(game.players[&1].respawn_ticks, 0);
    }

    #[test]
    fn growing_earth_lifts_a_worm_standing_on_new_growth() {
        let mut game = Game::new(2);
        join(&mut game, 1, "lifted");
        game.tiles.fill(Tile::Air);
        for x in 1..(WIDTH - 1) as i32 {
            game.set_tile(x, HEIGHT as i32 - 1, Tile::Earth);
        }
        game.players.get_mut(&1).expect("worm exists").x = 50.0;
        game.players.get_mut(&1).expect("worm exists").y = HEIGHT as f32 - 2.0;
        for _ in 0..300 {
            game.grow_terrain();
            if game.tile(50, HEIGHT as i32 - 2) == Tile::Earth {
                break;
            }
        }

        assert_eq!(game.tile(50, HEIGHT as i32 - 2), Tile::Earth);
        assert!(game.players[&1].y < HEIGHT as f32 - 2.0);
    }

    #[test]
    fn terrain_growth_respects_the_cap() {
        let mut game = Game::new(3);
        game.tiles.fill(Tile::Earth);
        for cell in game.tiles.iter_mut().take(WIDTH * HEIGHT / 3) {
            *cell = Tile::Air;
        }
        let before = game.earth_count();
        assert!(before >= TERRAIN_CAP);

        game.grow_terrain();

        assert_eq!(game.earth_count(), before);
    }

    #[test]
    fn terrain_growth_reseeds_an_arena_after_all_earth_is_destroyed() {
        let mut game = Game::new(13);
        game.tiles.fill(Tile::Air);

        game.grow_terrain();

        assert!(
            game.earth_count() > 0,
            "a fully destroyed arena must be able to regrow terrain"
        );
    }

    #[test]
    fn critically_sparse_terrain_recovers_multiple_cells_per_growth_event() {
        let mut game = Game::new(14);
        game.tiles.fill(Tile::Air);
        game.set_tile(50, HEIGHT as i32 - 1, Tile::Earth);
        let before = game.earth_count();

        game.grow_terrain();

        assert!(
            game.earth_count() >= before + 4,
            "scarce terrain should regenerate in patches rather than one cell at a time"
        );
    }

    #[test]
    fn terrain_growth_ignores_floating_islands_and_restarts_from_the_floor() {
        let mut game = Game::new(15);
        game.tiles.fill(Tile::Air);
        for x in 1..(WIDTH - 1) as i32 {
            game.set_tile(x, 10, Tile::Earth);
        }

        game.grow_terrain();

        assert!(
            (1..(WIDTH - 1) as i32).all(|x| game.tile(x, 9) == Tile::Air),
            "unsupported islands must not serve as roots for new terrain"
        );
        assert!(
            (1..(WIDTH - 1) as i32).any(|x| game.tile(x, HEIGHT as i32 - 1) == Tile::Earth),
            "new ground must be rooted at the floor"
        );
    }

    #[test]
    fn floating_terrain_collapses_to_supported_ground_during_simulation() {
        let mut game = Game::new(16);
        game.tiles.fill(Tile::Air);
        game.set_tile(44, 4, Tile::Earth);
        game.set_tile(44, 9, Tile::Earth);
        game.next_growth_tick = u64::MAX;

        for _ in 0..(HANG_FALL_DELAY as i32 + HEIGHT as i32 * 2) {
            game.tick();
        }

        let earth_on_floor = (0..WIDTH as i32)
            .filter(|&x| game.tile(x, HEIGHT as i32 - 1) == Tile::Earth)
            .count();
        assert!(earth_on_floor >= 2);
        assert_eq!(game.tile(44, 4), Tile::Air);
        assert_eq!(game.tile(44, 9), Tile::Air);
    }

    #[test]
    fn authoritative_battlefield_uses_four_physics_cells_per_native_terminal_cell() {
        assert_eq!(WIDTH, 77 * 4);
        assert_eq!(HEIGHT, 18 * 4);
    }

    #[test]
    fn grounded_worm_slides_down_a_surface_steeper_than_thirty_degrees() {
        let mut game = Game::new(17);
        join(&mut game, 1, "slider");
        game.tiles.fill(Tile::Air);
        for x in 0..WIDTH as i32 {
            let surface = if x < 44 { 30 } else { 34 };
            for y in surface..HEIGHT as i32 {
                game.set_tile(x, y, Tile::Earth);
            }
        }
        let player = game.players.get_mut(&1).expect("slider exists");
        player.x = 43.0;
        player.y = 29.0;
        player.vx = 0.0;

        game.tick();

        assert!(
            game.players[&1].vx > 0.0,
            "a supported steep drop to the right should induce downhill sliding"
        );
    }

    #[test]
    fn rhythmic_direction_taps_build_more_velocity_than_slow_taps() {
        let mut game = Game::new(4);
        join(&mut game, 1, "rhythm");
        game.handle_input(1, b"d");
        game.tick();
        game.handle_input(1, b"d");
        let fast_impulse = game.players[&1].move_impulse;

        let mut slow = Game::new(4);
        join(&mut slow, 1, "rhythm");
        slow.handle_input(1, b"d");
        for _ in 0..9 {
            slow.tick();
        }
        slow.handle_input(1, b"d");
        let slow_impulse = slow.players[&1].move_impulse;

        assert!(fast_impulse > slow_impulse);
    }

    #[test]
    fn weapon_power_depends_on_elapsed_held_enter_time() {
        let mut quick = Game::new(5);
        join(&mut quick, 1, "quick");
        quick.tiles.fill(Tile::Air);
        quick.players.get_mut(&1).expect("worm exists").y = 3.0;
        quick.handle_input(1, b"\r");
        for _ in 0..=FIRE_RELEASE_SILENCE {
            quick.tick();
        }
        let quick_speed = quick.projectiles[0].vx.abs();

        let mut charged = Game::new(5);
        join(&mut charged, 1, "charged");
        charged.tiles.fill(Tile::Air);
        charged.players.get_mut(&1).expect("worm exists").y = 3.0;
        charged.handle_input(1, b"\r");
        for _ in 0..12 {
            charged.tick();
            charged.handle_input(1, b"\r");
        }
        for _ in 0..=FIRE_RELEASE_SILENCE {
            charged.tick();
        }
        let charged_speed = charged.projectiles[0].vx.abs();

        assert!(charged_speed > quick_speed);
    }

    #[test]
    fn repeated_enter_packets_do_not_inflate_power_without_elapsed_time() {
        let mut single = Game::new(6);
        join(&mut single, 1, "single");
        single.tiles.fill(Tile::Air);
        single.players.get_mut(&1).expect("worm exists").y = 3.0;
        single.handle_input(1, b"\r");
        single.tick();
        single.handle_input(1, b"\r");
        for _ in 0..=FIRE_RELEASE_SILENCE {
            single.tick();
        }

        let mut spammed = Game::new(6);
        join(&mut spammed, 1, "spammed");
        spammed.tiles.fill(Tile::Air);
        spammed.players.get_mut(&1).expect("worm exists").y = 3.0;
        spammed.handle_input(1, b"\r\r\r\r\r");
        spammed.tick();
        spammed.handle_input(1, b"\r\r\r\r\r");
        for _ in 0..=FIRE_RELEASE_SILENCE {
            spammed.tick();
        }

        assert_eq!(single.projectiles[0].vx, spammed.projectiles[0].vx);
    }

    #[test]
    fn multiple_movement_bytes_in_one_tick_do_not_inflate_acceleration() {
        let mut one = Game::new(7);
        join(&mut one, 1, "one");
        one.handle_input(1, b"d");

        let mut repeated = Game::new(7);
        join(&mut repeated, 1, "repeated");
        repeated.handle_input(1, b"dddddd");

        assert_eq!(
            one.players[&1].move_impulse,
            repeated.players[&1].move_impulse
        );
    }

    #[test]
    fn aiming_does_not_clear_velocity_or_active_charge() {
        let mut game = Game::new(8);
        join(&mut game, 1, "pilot");
        game.handle_input(1, b"d\r");
        game.tick();
        let before_velocity = game.players[&1].vx;
        let before_charge = game.players[&1].charge_started;

        game.handle_input(1, b"jj");

        assert_eq!(game.players[&1].vx, before_velocity);
        assert_eq!(game.players[&1].charge_started, before_charge);
        assert_eq!(game.players[&1].aim, -2);
    }
}
