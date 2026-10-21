use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse;
use axum::{
    Router,
    extract::ws::{Message, WebSocket},
    routing::get,
    extract::State,
};
use futures_util::{SinkExt, StreamExt};
use protocol::{ClientMessage, ServerMessage, zone_at};
use tokio::sync::broadcast;
use std::collections::HashSet;

struct Player {
    name: String,
    x: i32,
    y: i32,
    status: String,
}

#[derive(Clone)]
struct AppState {
    players: Arc<Mutex<HashMap<String, Player>>>,
    tx: broadcast::Sender<String>,
    invite_codes: HashSet<String>,
}

fn load_invite_codes() -> HashSet<String> {
    std::env::var("INVITE_CODES")
        .unwrap_or_default()
        .split(',')
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect()
}

async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    // ws.on_failed_upgrade(|error| println!("Error upgrading websocket: {}", error))
    //     .on_upgrade(handle_socket)
    ws.on_upgrade(move |s| handle_socket(s, state.clone()))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    // Returns None when the stream is closed.
    // Send welcome message to the new joiner.
    let id = uuid::Uuid::new_v4().to_string();
    let welcome = ServerMessage::Welcome { id: id.clone() };
    let text = serde_json::to_string(&welcome).unwrap();
    let mut rx = state.tx.subscribe();
    let ( mut sender, mut receiver ) = socket.split();
    let _ = sender.send(Message::Text(text.into())).await;
    loop {
        tokio::select! {
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let parsed_message: Result<ClientMessage, _> = serde_json::from_str(&text.to_string());
                        match parsed_message {
                            Ok(ClientMessage::Join {name, code}) => {
                                // Checking invite code 
                                if !state.invite_codes.contains(&code) {
                                    let msg = serde_json::to_string(&ServerMessage::Rejected {
                                        reason: "invalid invite code".into(),
                                    }).unwrap();
                                    let _ = sender.send(Message::Text(msg.into())).await;
                                    return
                                }
                                println!("player joined");
                                // lock players, insert Player.
                                state.players.lock().unwrap().insert(id.clone(), Player { name: name.clone(), x: 0, y: 0, status: "online".to_string() });
                                let snapshot: Vec<ServerMessage> = {
                                    let players = state.players.lock().unwrap();
                                    players.iter().filter(|(pid, _)| pid.as_str() != id.as_str()).map(|(pid, p)| ServerMessage::Presence {
                                        id: pid.clone(), name: p.name.clone(), x: p.x, y: p.y, status: p.status.clone()
                                    }).collect()
                                };
                                // broadcast player joining
                                let broadcastMessage = ServerMessage::Presence { id: id.clone(), name: name.clone(), x: 0, y: 0, status: "online".to_string() };
                                let text = serde_json::to_string(&broadcastMessage).unwrap();
                                let _ = state.tx.send(text);
                                for msg in snapshot {
                                    let txt = serde_json::to_string(&msg).unwrap();
                                    if sender.send(Message::Text(txt.into())).await.is_err() { break; }
                                }
                            }
                            Ok(ClientMessage::Move {x, y}) => {
                                let mut players = state.players.lock().unwrap();
                                if let Some(p) = players.get_mut(&id) {
                                    p.x = x; p.y = y;
                                    let out = ServerMessage::Presence {
                                        id: id.clone(), name: p.name.clone(),
                                        x: p.x, y: p.y, status: p.status.clone(),
                                    };
                                    let _ = state.tx.send(serde_json::to_string(&out).unwrap());
                                }
                            }
                            Ok(ClientMessage::Chat {message}) => {
                                let Some((from, zone)) = state.players.lock().unwrap().get(&id).map(|p| {( p.name.clone(), zone_at(p.x) )}) else { continue; };
                                let out = ServerMessage::Chat {from, message, zone};
                                let _ = state.tx.send(serde_json::to_string(&out).unwrap());
                            }
                            Ok(ClientMessage::SetStatus {status}) => {
                                let mut players = state.players.lock().unwrap();
                                if let Some(p) = players.get_mut(&id) {
                                    p.status = status.clone();
                                    let out = ServerMessage::Presence { id: id.clone(), name: p.name.clone(), x: p.x, y: p.y, status: p.status.clone()};
                                    let _ = state.tx.send(serde_json::to_string(&out).unwrap());
                                }

                            }
                            Err(e) => eprintln!("bad client message: {e}"),
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
                            }
            msg = rx.recv() => {
                match msg {
                    Ok(broadcast_text) => {
                        let parsed: Result<ServerMessage, _> = serde_json::from_str(&broadcast_text);  
                        match parsed {
                            Ok(ServerMessage::Chat {zone, ..}) => {
                                let same_zone = state.players.lock().unwrap().get(&id).map(|p| {zone_at(p.x) == zone}).unwrap_or(false);
                                if !same_zone {
                                    continue;
                                }
                                if sender.send(Message::Text(broadcast_text.into())).await.is_err() {
                                    break;
                                }
                            }
                            Ok(_) => {
                                // Presence or player left
                                if sender.send(Message::Text(broadcast_text.into())).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => {
                                // leave the string alone and still send it.
                                if sender.send(Message::Text(broadcast_text.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                        
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break, // channel closed
                }
            }
        }
    }
    // cleanup on disconnect
    state.players.lock().unwrap().remove(&id);
    let left = ServerMessage::PlayerLeft { id: id.clone() };
    let _ = state.tx.send(serde_json::to_string(&left).unwrap());
}

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{port}");
    let (tx, _) = broadcast::channel(32);
    let invite_codes = load_invite_codes();
    let state = AppState { players: Arc::new(Mutex::new(HashMap::new())), tx, invite_codes};
    let app = Router::new().route("/ws", get(ws_handler)).with_state(state);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();

}
