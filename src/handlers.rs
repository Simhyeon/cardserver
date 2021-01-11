use std::convert::Infallible;
use std::sync::{ Arc , Mutex};
use std::collections::HashMap;
use tokio::sync::mpsc;
use uuid::Uuid;
use warp::{Filter, Reply};
use futures::{FutureExt, StreamExt};
use warp::ws::{Message, WebSocket};

use crate::models::{Connection, UserRequest, PlayerAction, ServerResponse, ResponseType, ResponseValue, InternalRequest, IntReqType, IntReqValue};

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
    let (internal_tx, mut internal_rx) = mpsc::unbounded_channel();

    // Create user id and insert into connetion hashmap
    let user_id = Uuid::new_v4().to_simple().to_string();
    let room_id = Uuid::new_v4().to_simple().to_string();

    let msg = serde_json::to_string(&ServerResponse{
            response_type: ResponseType::Message, 
            value: ResponseValue::Message(format!("Successfully created a room : {}", room_id).to_string())})
        .expect("Failed to create json object");

    server_tx.send(Ok(Message::text(msg))).expect("Failed to send message");

    conn.lock().unwrap().insert(room_id.clone(), Connection::new(user_id.clone(), room_id.clone(), server_tx, internal_tx));


    tokio::task::spawn( server_rx.forward(user_tx).map(|result| {
        if let Err(e) = result {
            eprintln!("websocket error: {:?}", e);
        }
    }));
    
    // Create new task so that internal channel and
    // user channel are asynchronously recived from server.
    let room_id_clone = room_id.clone();
    let conn_clone = conn.clone();
    tokio::task::spawn(
        async move{
            while let Some(result) = internal_rx.next().await {
                eprintln!("Internal message");
                let msg = match result {
                    Ok(msg) => msg,
                    Err(e) => {
                        eprintln!("websocket error {}", e);
                        break;
                    }
                };
                internal_request_handler(&room_id_clone, msg, &conn_clone).await;
            }
        }
    );

    while let Some(result) = user_rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error {}", e);
                break;
            }
        };
        user_request_handler(&room_id, &user_id, msg, &conn).await;
    }

    user_disconnected_handler(&user_id, &conn).await;
}

pub async fn join(ws: WebSocket, room_id: String, conn: Connections) {
    let (user_tx, mut user_rx) = ws.split();
    let (server_tx, server_rx) = mpsc::unbounded_channel();

    // Create user id and insert into connetion hashmap
    let user_id = Uuid::new_v4().to_simple().to_string();

    // SPawn a thread for forwarding stream from server reciever to user transmitter.
    tokio::task::spawn( server_rx.forward(user_tx).map(|result| {
        if let Err(e) = result {
            eprintln!("websocket error: {:?}", e);
        }
    }));

    if let Some(connection) = conn.lock().unwrap().get_mut(&room_id) {

        let msg = serde_json::to_string(&ServerResponse{
            response_type: ResponseType::Message, 
            value: ResponseValue::Message(format!("Successfully joined a room : {}", room_id).to_string())})
            .expect("Failed to create json object");

        server_tx.send(Ok(Message::text(msg))).expect("Failed to send message");
        // Set connection into room
        connection.game.join_game(user_id.clone(), server_tx);
        // Initialize game.
        // Which make community field and hand of each players 
        // And also sends card information to each clients.
        connection.game.init_game();
    } else {
        // Reject
        let msg = serde_json::to_string(&ServerResponse{
            response_type: ResponseType::Message, 
            value: ResponseValue::Message("Failed to join room".to_string())})
            .expect("Failed to create json object");

        server_tx.send(Ok(Message::text(msg))).expect("Failed to send message");
        return;
    }

    // TODO :: Change this opertion from insert into modification.
    //conn.lock().unwrap().insert(user_id.clone(), Connection::new(user_id.clone(), room_id, server_tx));


    // From user client to server receiver.
    while let Some(result) = user_rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error {}", e);
                break;
            }
        };
        user_request_handler(&room_id, &user_id, msg, &conn).await;
        //user_message(&room_id, &user_id, msg, &conn).await;
    }

    user_disconnected_handler(&room_id, &conn).await;
}

pub async fn internal_request_handler(room_id: &str, msg: Message, conn: &Connections) {
    // Skip any non-Text messages...
    let msg = if let Ok(s) = msg.to_str() {
        s
    } else {
        return;
    };

    let mut req: InternalRequest = InternalRequest::dummy();
    if let Ok(request) = serde_json::from_str(msg) {
        req = request;
    } else {
        eprintln!("Failed to parse Internal Request");
        eprintln!("{}", msg);
       return; 
    }
    eprintln!("Successfully fetched internal request of type : {:?}", req.request_type);

    match req.request_type {
        // NOTE
        // Timeout is only nested one time.
        // Which means timeout exception can only occur once.
        // Such that internal_request doesn't have to wait for multiple
        // times before completion.
        IntReqType::TimeOut => {
            if let IntReqValue::TimeOut(time_out) = req.value {
                eprintln!("Wait for {:?} seconds", time_out.duration);

                // Create a new task, namely concurrent workflow
                // so that timeout delay is done asynchronously
                // while internal_channel can recieve another request
                let conn_clone = conn.clone();
                let room_id_clone = room_id.clone().to_string();
                tokio::task::spawn(async move{
                    tokio::time::delay_for(std::time::Duration::from_secs(time_out.duration.as_secs())).await;

                    // Get conection
                    let mut hash = conn_clone.lock().unwrap();
                    if let Some(connection) = hash.get_mut(&room_id_clone) {
                        // If state has been extended ignore request and drop.
                        connection.game.next_state(&time_out.state_id);
                    } else {
                        eprintln!("Skipped request because connection lost");
                    }
                });
            } else {
                eprintln!("Cannot not find duration value from timeout request");
            }
        }
        _ => {}
    }
}

pub async fn user_request_handler(room_id: &str, user_id: &str, msg: Message, conn: &Connections) {
    // Skip any non-Text messages...
    let msg = if let Ok(s) = msg.to_str() {
        s
    } else {
        return;
    };

    let mut req: UserRequest = UserRequest::dummy();
    if let Ok(request) = serde_json::from_str(msg) {
        req = request;
    } else {
        eprintln!("Failed to parse userrequest");
        eprintln!("{}", msg);
       return; 
    }

    eprintln!("Received user request");
    eprintln!("{:?}", req);
    let mut hash = conn.lock().unwrap();
    if let Some(connection) = hash.get_mut(room_id) {
        // New message from this user, send it to everyone else (except same uid)...
        let pending = connection.game.receive_player_action(&user_id, req);
        connection.game.pending_next_state(pending);
    } else {
        eprintln!("Connection lost");
    }
}

pub async fn user_disconnected_handler(room_id: &str, conn: &Connections) {
    eprintln!("User disconnected");

    // Stream closed up, so remove from the user list
    conn.lock().unwrap().remove(room_id);
}

pub fn with_conns(conn: Connections) -> impl Filter<Extract = (Connections,), Error = Infallible> + Clone {
    warp::any().map(move || conn.clone())
}
