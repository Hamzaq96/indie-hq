use std::time::Duration;

use crossterm::event::{self, Event, KeyCode};
use futures_util::{SinkExt, StreamExt};
use ratatui::{Frame, layout::Rect, widgets::{Block, Paragraph}};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use protocol::{ClientMessage, ServerMessage};
use tokio::sync::mpsc;
use std::collections::HashMap;
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
    Focus, Open, Office
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
    }
    else if x < BOUND2 {
        Zone::Open
    }
    else {
        Zone::Office
    }
}

impl App {
    const MAX_X: u16 = MAP_W - 3;
    const MAX_Y: u16 = MAP_H -3;

    fn new() -> Self {
        Self { id: None, x: 5, y: 5, others: HashMap::new(), lines: Vec::new(), typing: false, draft: String::new() }
    }

    fn current_zone(&self) -> Zone {
        zone_at(self.x, self.y)
    }

    fn on_key(&mut self, code: KeyCode) -> bool {
        // TODO: match WASD / arrows -> change self.x / self.y
        // TODO: what stops avatar leaving the map? clamp?
        match code {
            KeyCode::Char('w') | KeyCode::Up => {
                self.y = self.y.saturating_sub(1);
                return true;
            }
            KeyCode::Char('s') | KeyCode::Down=> {
                self.y = (self.y.saturating_add(1)).min(Self::MAX_Y);
                return true;
            }
            KeyCode::Char('a') | KeyCode::Left => {
                self.x = self.x.saturating_sub(1);
                return true;
            }
            KeyCode::Char('d') | KeyCode::Right => {
                self.x = (self.x.saturating_add(1)).min(Self::MAX_X);
                return true;
            }
            _ => { return false }
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
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
            None => format!("indie-hq [{:?}]", self.current_zone())
        };
        // TODO: Block with Borders titled "indie-hq"
        // TODO: Paragraph with "@" rendered at (self.x, self.y) inside area?
        // Hint: look at `frame.render_widget(widget, area)` + `Rect::new(x, y, 1, 1)`
        frame.render_widget(Block::bordered().title(title), centered);
        let label_y = centered.y + 1;
        frame.render_widget(Paragraph::new("FOCUS"), Rect::new(centered.x + 1 + 2, label_y, 5, 1));
        frame.render_widget(Paragraph::new("OPEN"), Rect::new(centered.x + 1 + 12, label_y, 5, 1));
        frame.render_widget(Paragraph::new("OFFICE"), Rect::new(centered.x + 1 + 22, label_y, 5, 1));
        for dx in [BOUND1, BOUND2] {
            for dy in 0..MAP_H-2 {
                frame.render_widget(Paragraph::new("|"), Rect::new(centered.x + 1 + dx, centered.y + 1 + dy, 1, 1));
            }
        }
        frame.render_widget(Paragraph::new("@"), Rect::new(centered.x + 1 + self.x, centered.y + 1 + self.y, 1, 1));

        for ( _id, (ox, oy)) in &self.others {
            frame.render_widget(Paragraph::new("o"), Rect::new(centered.x + 1 + ox, centered.y + 1 + oy, 1, 1));
        }

        let start = self.lines.len().saturating_sub(5);
        let mut text = self.lines[start..].join("\n");
        if self.typing {
            text.push_str(&format!("\n> {}", self.draft));
        }
        frame.render_widget(Paragraph::new(text), Rect::new(centered.x, centered.y + centered.height, centered.width, 5))
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // TODO 1: crossterm terminal init - enable_raw_mode? EnterAlternateScreen?
    let mut terminal = ratatui::init();
    let name = std::env::var("INDIE_HQ_NAME").expect("set INDIE_HQ_NAME");
    let url = std::env::var("INDIE_HQ_URL").unwrap_or_else(|_| "ws://127.0.0.1:3000/ws".to_string());
    let (ws_stream, _) = connect_async(url).await.expect("connect failed");
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
    // TODO 2: loop { terminal.draw(|f| app.draw(f))?; if event::poll(...)? { if let Event::Key(k) = event::read()? { if k.code == KeyCode::Char('q') { break } else { app.on_key(k.code) } } }
    loop {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ServerMessage::Welcome { id } => app.id = Some(id),
                ServerMessage::Presence { id, name: _, x, y, status: _ } => {
                    // Skip self echo
                    if app.id.as_ref() == Some(&id) { continue; }
                    let ux = (x.max(0) as u16).min(App::MAX_X);
                    let uy = (y.max(0) as u16).min(App::MAX_Y);
                    app.others.insert(id, (ux, uy));
                },
                ServerMessage::PlayerLeft { id } => { 
                    app.others.remove(&id);
                }
                ServerMessage::Chat { from, message, .. } => {
                    app.lines.push(format!("{from}: {message}"));
                }
                _ => {}
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
                                let msg = serde_json::to_string(&ClientMessage::Chat { message: app.draft.clone(), }).unwrap();
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
                }
                else if k.code == KeyCode::Char('q') {
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
                        let txt = serde_json::to_string(&ClientMessage::Move { x: app.x as i32, y: app.y as i32 }).unwrap();
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
