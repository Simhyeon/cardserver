use std::convert::Infallible;
use std::sync::{ Arc , Mutex};
use std::collections::HashMap;
use tokio::sync::mpsc;
use uuid::Uuid;
use warp::{Filter, Reply};
use futures::{FutureExt, StreamExt};
use warp::ws::{Message, WebSocket};

use crate::models::{Connection, UserRequest, PlayerAction, ServerResponse, ResponseType, ResponseValue};

pub type Connections = Arc<Mutex<HashMap<String, Connection>>>;

// This conn is given as clone object so that it is alright to just move conn to nested functions
pub async fn create_handler(ws: warp::ws::Ws, conn: Connections) -> Result<impl Reply, Infallible> {
    Ok( ws.on_upgrade(move |ws| create(ws, conn) ))
}

pub async fn join_handler(ws: warp::ws::Ws, room_id: String,conn: Connections) -> Result<impl Reply, Infallible> {
    Ok( ws.on_upgrade(move |ws| join(ws, room_id, conn) ))
}

pub async fn create(ws: WebSocket, conn: Connections) {
    let (user_tx, mut user_rx) = ws.split();
    let (server_tx, server_rx) = mpsc::unbounded_channel();

    // Create user id and insert into connetion hashmap
    let user_id = Uuid::new_v4().to_simple().to_string();
    let room_id = Uuid::new_v4().to_simple().to_string();

    let msg = serde_json::to_string(&ServerResponse{
            response_type: ResponseType::Message, 
            value: ResponseValue::Message(format!("Successfully created a room : {}", room_id).to_string())})
        .expect("Failed to create json object");

    server_tx.send(Ok(Message::text(msg))).expect("Failed to send message");

    conn.lock().unwrap().insert(room_id.clone(), Connection::new(user_id.clone(), room_id.clone(), server_tx));


    tokio::task::spawn( server_rx.forward(user_tx).map(|result| {
        if let Err(e) = result {
            eprintln!("websocket error: {:?}", e);
        }
    }));

    while let Some(result) = user_rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error {}", e);
                break;
            }
        };
        user_request(&room_id, &user_id, msg, &conn).await;
        //user_message(&room_id, &user_id, msg, &conn).await;
    }

    user_disconnected(&user_id, &conn).await;
}

pub async fn join(ws: WebSocket, room_id: String, conn: Connections) {
    let (user_tx, mut user_rx) = ws.split();
    let (server_tx, server_rx) = mpsc::unbounded_channel();

    // Create user id and insert into connetion hashmap
    let user_id = Uuid::new_v4().to_simple().to_string();

    let msg = serde_json::to_string(&ServerResponse{
            response_type: ResponseType::Message, 
            value: ResponseValue::Message(format!("Successfully joined a room : {}", room_id).to_string())})
        .expect("Failed to create json object");

    server_tx.send(Ok(Message::text(msg))).expect("Failed to send message");

    // TODO :: Change this opertion from insert into modification.
    //conn.lock().unwrap().insert(user_id.clone(), Connection::new(user_id.clone(), room_id, server_tx));
    if let Some(connection) = conn.lock().unwrap().get_mut(&room_id) {
        // Set connection into room
        connection.game.join_game(user_id.clone(), server_tx);
    } else {
        // Reject
    }

    tokio::task::spawn( server_rx.forward(user_tx).map(|result| {
        if let Err(e) = result {
            eprintln!("websocket error: {:?}", e);
        }
    }));


    // From user client to server receiver.
    while let Some(result) = user_rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error {}", e);
                break;
            }
        };
        user_request(&room_id, &user_id, msg, &conn).await;
        //user_message(&room_id, &user_id, msg, &conn).await;
    }

    user_disconnected(&user_id, &conn).await;
}

pub async fn user_request(room_id: &str, user_id: &str, msg: Message, conn: &Connections) {
    // Skip any non-Text messages...
    let msg = if let Ok(s) = msg.to_str() {
        s
    } else {
        return;
    };

    let mut req: UserRequest = UserRequest{action: PlayerAction::Message, value: None};
    if let Ok(request) = serde_json::from_str(msg) {
        req = request;
    } else {
        eprintln!("Failed to parse userrequest");
        eprintln!("{}", msg);
       return; 
    }

    // New message from this user, send it to everyone else (except same uid)...
    conn.lock().unwrap().get_mut(room_id).unwrap()
        .game.receive_player_action(&user_id, req.action);
}

pub async fn _user_message(room_id: &str, user_id: &str, msg: Message, conn: &Connections) {
    // Skip any non-Text messages...
    let msg = if let Ok(s) = msg.to_str() {
        s
    } else {
        return;
    };

    let new_msg = format!("From {}, Message : {}", user_id, msg);

    // New message from this user, send it to everyone else (except same uid)...
    conn.lock().unwrap().get(room_id).unwrap()
        .game.send_message(user_id.to_string(), Message::text(new_msg));
}

pub async fn user_disconnected(user_id: &str, conn: &Connections) {
    eprintln!("good bye user: {}", user_id);

    // Stream closed up, so remove from the user list
    conn.lock().unwrap().remove(user_id);
}

pub fn with_conns(conn: Connections) -> impl Filter<Extract = (Connections,), Error = Infallible> + Clone {
    warp::any().map(move || conn.clone())
}
