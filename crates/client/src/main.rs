use std::time::Duration;

use crossterm::event::{self, Event, KeyCode};
use futures_util::{SinkExt, StreamExt};
use protocol::{ClientMessage, ServerMessage};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::Paragraph,
};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
// TODO: what do you need from protocol? ClientMessage? ServerMessage?

const MAP_W: u16 = 30;
const MAP_H: u16 = 12;
const BOUND1: u16 = 10;
const BOUND2: u16 = 20;
const FOCUS_URL: &str = "https://discord.gg/placeholder-focus";
const OPEN_URL: &str = "https://discord.gg/placeholder-open";
const OFFICE_URL: &str = "https://discord.gg/placeholder-office";
const CAL_URL: &str = "https://cal.com/placeholder-calendar";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zone {
    Focus,
    Open,
    Office,
}

struct App {
    id: Option<String>,
    // name: string,
    x: u16,
    y: u16,
    // status: string,
    others: HashMap<String, (u16, u16)>,
    lines: Vec<String>,
    typing: bool,
    draft: String,
}

fn discord_url(zone: Zone) -> &'static str {
    match zone {
        Zone::Focus => FOCUS_URL,
        Zone::Open => OPEN_URL,
        Zone::Office => OFFICE_URL,
    }
}

fn zone_at(x: u16, _y: u16) -> Zone {
    if x < BOUND1 {
        Zone::Focus
    } else if x < BOUND2 {
        Zone::Open
    } else {
        Zone::Office
    }
}

fn is_wall(x: u16, y: u16) -> bool {
   let on_divider = x == BOUND1 || x == BOUND2;
   let in_door = (4..=6).contains(&y);
   on_divider && !in_door
}

fn map_grid() -> Vec<String> {
    let mut rows = Vec::new();
    for row in 0..MAP_H {
        let mut line = Vec::with_capacity(MAP_W as usize);
        for col in 0..MAP_W {
            let ch = if row == 0 {
                match col {
                    0 => '┌',
                    11 | 21 => '┬',
                    29 => '┐',
                    _ => '─',
                }
            } else if row == MAP_H - 1 {
                match col {
                    0 => '└',
                    11 | 21 => '┴',
                    29 => '┘',
                    _ => '─',
                }
            } else if col == 0 || col == MAP_W - 1 {
                '│'
            } else if is_wall(col - 1, row - 1) {
                '│'
            } else {
                ' '
            };
            line.push(ch);
        }
        if row == 1 {
            for (i, ch) in "FOCUS".chars().enumerate() {
                line[3 + i] = ch;
            }
            for (i, ch) in "OPEN".chars().enumerate() {
                line[13 + i] = ch;
            }
            for (i, ch) in "OFFICE".chars().enumerate() {
                line[23 + i] = ch;
            }
        }
        rows.push(line.into_iter().collect());
    }
    rows
}

fn face_for(id: &str) -> char {
    const FACES: &[char]= &['☺', '☻', '♠', '♣', '♥', '♦', '▲', '●'];
    let n = id.bytes().fold(0usize, |acc, b| acc.wrapping_add(b as usize));
    FACES[n % FACES.len()]
}

impl App {
    const MAX_X: u16 = MAP_W - 3;
    const MAX_Y: u16 = MAP_H - 3;

    fn new() -> Self {
        Self {
            id: None,
            x: 5,
            y: 5,
            others: HashMap::new(),
            lines: Vec::new(),
            typing: false,
            draft: String::new(),
        }
    }

    fn current_zone(&self) -> Zone {
        zone_at(self.x, self.y)
    }

    fn on_key(&mut self, code: KeyCode) -> bool {
        // TODO: match WASD / arrows -> change self.x / self.y
        // TODO: what stops avatar leaving the map? clamp?
        let (nx, ny) = match code {
            KeyCode::Char('w') | KeyCode::Up => (self.x, self.y.saturating_sub(1)),
            KeyCode::Char('s') | KeyCode::Down => {
                (self.x, (self.y.saturating_add(1)).min(Self::MAX_Y))
            }
            KeyCode::Char('a') | KeyCode::Left => (self.x.saturating_sub(1), self.y),
            KeyCode::Char('d') | KeyCode::Right => {
                ((self.x.saturating_add(1)).min(Self::MAX_X), self.y)
            }
            _ => return false,
        };
        if is_wall(nx, ny) {
            return false;
        }

        self.x = nx;
        self.y = ny;
        true
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        let face = match &self.id {
            Some(id) => face_for(id),
            None => '☺',
        };
        let centered = Rect::new(
            area.width.saturating_sub(MAP_W) / 2,
            area.height.saturating_sub(MAP_H) / 2,
            MAP_W,
            MAP_H,
        );
        let title = match &self.id {
            Some(id) => {
                let short: String = id.chars().take(8).collect();
                format!("indie-hq [{:?}] {}", self.current_zone(), short)
            }
            None => format!("indie-hq [{:?}]", self.current_zone()),
        };
        if centered.y > 0 {
            frame.render_widget(
                Paragraph::new(title),
                Rect::new(centered.x, centered.y - 1, centered.width, 1),
            );
        }
        let grid = map_grid();
        frame.render_widget(Paragraph::new(grid.join("\n")), centered);
        frame.render_widget(
            Paragraph::new(face.to_string()),
            Rect::new(centered.x + 1 + self.x, centered.y + 1 + self.y, 1, 1),
        );

        for (id, (ox, oy)) in &self.others {
            frame.render_widget(
                Paragraph::new(face_for(id).to_string()),
                Rect::new(centered.x + 1 + ox, centered.y + 1 + oy, 1, 1),
            );
        }

        let start = self.lines.len().saturating_sub(5);
        let mut text = self.lines[start..].join("\n");
        if self.typing {
            text.push_str(&format!("\n> {}", self.draft));
        }
        frame.render_widget(
            Paragraph::new(text),
            Rect::new(centered.x, centered.y + centered.height, centered.width, 5),
        )
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // TODO 1: crossterm terminal init - enable_raw_mode? EnterAlternateScreen?
    let name = std::env::var("INDIE_HQ_NAME")
        .or_else(|_| std::env::var("USER"))
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "guest".to_string());
    let url = std::env::var("INDIE_HQ_URL")
        .unwrap_or_else(|_| "wss://indie-hq-production.up.railway.app/ws".to_string());
    let (ws_stream, _) = match connect_async(&url).await {
        Ok(ws_stream) => ws_stream,
        Err(e) => {
            eprintln!("connect failed: {e}");
            return Ok(());
        }
    };
    let (mut ws_sender, mut ws_reader) = ws_stream.split();
    let join = ClientMessage::Join { name: name.clone() };
    let txt = serde_json::to_string(&join).unwrap();
    ws_sender.send(Message::Text(txt.into())).await.unwrap();
    // channel + spawn
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();
    tokio::spawn(async move {
        while let Some(msg) = ws_reader.next().await {
            match msg {
                Ok(Message::Text(txt)) => {
                    match serde_json::from_str::<ServerMessage>(&txt.to_string()) {
                        Ok(parsed) => {
                            let _ = tx.send(parsed);
                        }
                        Err(e) => eprintln!("bad ServerMessage: {e}"),
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(_) => {} // Binary, Ping, Pong - ignore for now.
                Err(e) => {
                    eprintln!("ws read err: {e}");
                    break;
                }
            }
        }
    });

    let mut app = App::new();
    let mut terminal = ratatui::init();
    // TODO 2: loop { terminal.draw(|f| app.draw(f))?; if event::poll(...)? { if let Event::Key(k) = event::read()? { if k.code == KeyCode::Char('q') { break } else { app.on_key(k.code) } } }
    loop {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ServerMessage::Welcome { id } => app.id = Some(id),
                ServerMessage::Presence {
                    id,
                    name: _,
                    x,
                    y,
                    status: _,
                } => {
                    // Skip self echo
                    if app.id.as_ref() == Some(&id) {
                        continue;
                    }
                    let ux = (x.max(0) as u16).min(App::MAX_X);
                    let uy = (y.max(0) as u16).min(App::MAX_Y);
                    app.others.insert(id, (ux, uy));
                }
                ServerMessage::PlayerLeft { id } => {
                    app.others.remove(&id);
                }
                ServerMessage::Chat { from, message, .. } => {
                    app.lines.push(format!("{from}: {message}"));
                }
            }
        }
        terminal.draw(|frame| app.draw(frame))?;
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(k) = event::read()? {
                if app.typing {
                    match k.code {
                        KeyCode::Char(c) => {
                            app.draft.push(c);
                        }
                        KeyCode::Backspace => {
                            app.draft.pop();
                        }
                        KeyCode::Enter => {
                            if !app.draft.is_empty() {
                                let msg = serde_json::to_string(&ClientMessage::Chat {
                                    message: app.draft.clone(),
                                })
                                .unwrap();
                                ws_sender.send(Message::Text(msg.into())).await.unwrap();
                                app.draft.clear();
                                app.typing = false;
                            }
                        }
                        KeyCode::Esc => {
                            app.draft.clear();
                            app.typing = false;
                        }
                        _ => {}
                    }
                } else if k.code == KeyCode::Char('q') {
                    break;
                } else if k.code == KeyCode::Char('V') {
                    let url = discord_url(app.current_zone());
                    let _ = open::that(url);
                } else if k.code == KeyCode::Char('B') {
                    if app.current_zone() == Zone::Office {
                        let url = CAL_URL;
                        let _ = open::that(url);
                    }
                } else if k.code == KeyCode::Char('C') {
                    app.typing = true;
                } else {
                    if app.on_key(k.code) {
                        // build Move
                        let txt = serde_json::to_string(&ClientMessage::Move {
                            x: app.x as i32,
                            y: app.y as i32,
                        })
                        .unwrap();
                        // wrap the move in Message
                        ws_sender.send(Message::Text(txt.into())).await.unwrap();
                    }
                }
            }
        }
    }
    // TODO 3: restore terminal on exit
    ratatui::restore();
    // WS connect comes AFTER local movement works - ignore for now
    Ok(())
}
