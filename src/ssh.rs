use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use anyhow::Result;
use russh::{
    Channel, ChannelId, Pty,
    server::{Auth, Handler, Msg, Server, Session},
};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tracing::warn;

use crate::game::{ColorDepth, Event, GlyphMode, PlayerId};

const TERMINAL_ENTER: &str = "\x1b[?1049h\x1b[?25l\x1b[?1l\x1b[2J\x1b[H";
const TERMINAL_RESTORE: &str = "\x1b[0m\x1b[?25h\x1b[?1l\x1b[?1049l";

#[derive(Clone)]
pub struct WormServer {
    events: UnboundedSender<Event>,
    next_id: Arc<AtomicU64>,
}

impl WormServer {
    pub fn new(events: UnboundedSender<Event>) -> Self {
        Self {
            events,
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }
}

impl Server for WormServer {
    type Handler = WormSession;

    fn new_client(&mut self, _peer_addr: Option<SocketAddr>) -> Self::Handler {
        WormSession {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            events: self.events.clone(),
            username: None,
            channel: None,
            joined: false,
            columns: 102,
            rows: 41,
            colors: ColorDepth::Ansi16,
            truecolor: false,
            glyphs: None,
            awaiting_glyph_choice: false,
        }
    }
}

pub struct WormSession {
    id: PlayerId,
    events: UnboundedSender<Event>,
    username: Option<String>,
    channel: Option<ChannelId>,
    joined: bool,
    columns: u32,
    rows: u32,
    colors: ColorDepth,
    truecolor: bool,
    glyphs: Option<GlyphMode>,
    awaiting_glyph_choice: bool,
}

impl WormSession {
    fn leave(&mut self) {
        if self.joined {
            let _ = self.events.send(Event::Leave(self.id));
            self.joined = false;
        }
    }

    fn join_world(&mut self, channel: ChannelId, session: &mut Session) {
        let Some(username) = self.username.clone() else {
            return;
        };
        if session
            .data(channel, TERMINAL_ENTER.as_bytes().to_vec())
            .is_err()
        {
            return;
        }
        let (frames, mut frame_rx) = unbounded_channel::<String>();
        let handle = session.handle();
        tokio::spawn(async move {
            while let Some(frame) = frame_rx.recv().await {
                if handle.data(channel, frame.into_bytes()).await.is_err() {
                    break;
                }
            }
        });
        let _ = self.events.send(Event::Join {
            id: self.id,
            username,
            frames,
            columns: self.columns,
            rows: self.rows,
            colors: self.colors,
            glyphs: self.glyphs.unwrap_or(GlyphMode::Ascii),
        });
        self.joined = true;
        self.awaiting_glyph_choice = false;
    }
}

impl Handler for WormSession {
    type Error = anyhow::Error;

    async fn auth_none(&mut self, user: &str) -> Result<Auth, Self::Error> {
        self.username = Some(user.to_owned());
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        self.channel = Some(channel.id());
        Ok(true)
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        columns: u32,
        rows: u32,
        _pixel_width: u32,
        _pixel_height: u32,
        _modes: &[(Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.colors = color_depth(_term, self.truecolor.then_some("truecolor"));
        self.columns = columns;
        self.rows = rows;
        let _ = self.events.send(Event::Resize {
            id: self.id,
            columns,
            rows,
        });
        session.channel_success(channel)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.joined || self.channel != Some(channel) {
            session.channel_failure(channel)?;
            return Ok(());
        }
        if self.username.is_none() {
            session.channel_failure(channel)?;
            return Ok(());
        }
        self.awaiting_glyph_choice = true;
        session.data(
            channel,
            b"WORMS//SSH\r\nPowerlevel10k / Nerd Font icons available? [y/N] ".to_vec(),
        )?;
        session.channel_success(channel)?;
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.awaiting_glyph_choice && !self.joined {
            let response = String::from_utf8_lossy(data).to_ascii_lowercase();
            if response.contains('y') {
                self.glyphs = Some(GlyphMode::Powerlevel10k);
                self.join_world(channel, session);
            } else if response.contains('n') || response.contains('\r') || response.contains('\n') {
                self.glyphs = Some(GlyphMode::Ascii);
                self.join_world(channel, session);
            }
            return Ok(());
        }
        if data.contains(&3) {
            self.leave();
            session.data(
                channel,
                format!("{TERMINAL_RESTORE}\r\nThanks for playing.\r\n").into_bytes(),
            )?;
            session.close(channel)?;
            return Ok(());
        }
        let _ = self.events.send(Event::Input {
            id: self.id,
            input: data.to_vec(),
        });
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        columns: u32,
        rows: u32,
        _pixel_width: u32,
        _pixel_height: u32,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.columns = columns;
        self.rows = rows;
        let _ = self.events.send(Event::Resize {
            id: self.id,
            columns,
            rows,
        });
        session.channel_success(channel)?;
        Ok(())
    }

    async fn env_request(
        &mut self,
        channel: ChannelId,
        name: &str,
        value: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name == "COLORTERM" && value.to_ascii_lowercase().contains("truecolor") {
            self.truecolor = true;
            self.colors = ColorDepth::TrueColor;
            let _ = self.events.send(Event::Color {
                id: self.id,
                colors: self.colors,
            });
        }
        session.channel_success(channel)?;
        Ok(())
    }

    async fn channel_close(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.joined {
            let _ = session.data(channel, TERMINAL_RESTORE.as_bytes().to_vec());
        }
        self.leave();
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.joined {
            let _ = session.data(channel, TERMINAL_RESTORE.as_bytes().to_vec());
        }
        self.leave();
        Ok(())
    }
}

fn color_depth(term: &str, colorterm: Option<&str>) -> ColorDepth {
    if colorterm.is_some_and(|value| value.to_ascii_lowercase().contains("truecolor")) {
        ColorDepth::TrueColor
    } else if term.contains("256color") {
        ColorDepth::Ansi256
    } else if term == "dumb" {
        ColorDepth::Mono
    } else {
        ColorDepth::Ansi16
    }
}

impl Drop for WormSession {
    fn drop(&mut self) {
        if self.joined && self.events.send(Event::Leave(self.id)).is_err() {
            warn!("game loop dropped before SSH session cleanup");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_restore_leaves_the_alternate_screen_after_play() {
        assert!(
            TERMINAL_RESTORE.contains("\x1b[?1049l"),
            "leaving the arena must return the player's original terminal screen"
        );
    }

    #[test]
    fn entering_the_arena_hides_the_cursor_once_inside_the_alternate_screen() {
        assert!(
            TERMINAL_ENTER.contains("\x1b[?25l"),
            "the animation cursor must be hidden while the arena is visible"
        );
    }
}
