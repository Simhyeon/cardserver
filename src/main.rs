use std::sync::{ Arc , Mutex};
use std::convert::Infallible;
use std::collections::HashMap;
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc;
use warp::{Filter, Reply};
use uuid::Uuid;
use warp::ws::{Message, WebSocket};

type Connections = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Result<Message, warp::Error>>>>>;

#[tokio::main]
async fn main() {
    let conn = Connections::new(Mutex::new(HashMap::new()));
    let ws = warp::path("echo")
        // The `ws()` filter will prepare the Websocket handshake.
        .and(warp::ws())
        .and(with_conns(conn.clone()))
        .and_then(ws_handler);

    let routes = ws.with(warp::cors());

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

fn with_conns(conn: Connections) -> impl Filter<Extract = (Connections,), Error = Infallible> + Clone {
    warp::any().map(move || conn.clone())
}

pub async fn register_handler() {

}

// This conn is given as clone object so that it is alright to just move conn to nested functions
pub async fn ws_handler(ws: warp::ws::Ws, conn: Connections) -> Result<impl Reply, Infallible> {
    Ok( ws.on_upgrade(move |ws| connect(ws, conn) ))
}

pub async fn connect(ws: WebSocket, conn: Connections) {
    let (user_tx, mut user_rx) = ws.split();
    let (server_tx, server_rx) = mpsc::unbounded_channel();

    // Create user id and insert into connetion hashmap
    let user_id = Uuid::new_v4().to_simple().to_string();
    conn.lock().unwrap().insert(user_id.clone(), server_tx);

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
        user_message(&user_id, msg, &conn).await;
    }

    user_disconnected(&user_id, &conn).await;
}

pub async fn user_message(user_id: &str, msg: Message, conn: &Connections) {
    // Skip any non-Text messages...
    let msg = if let Ok(s) = msg.to_str() {
        s
    } else {
        return;
    };

    let new_msg = format!("From {}, Message : {}", user_id, msg);

    // New message from this user, send it to everyone else (except same uid)...
    for (uid, tx) in conn.lock().unwrap().iter() {
        if user_id != uid {
            if let Err(_disconnected) = tx.send(Ok(Message::text(new_msg.clone()))) {
                // The tx is disconnected, our `user_disconnected` code
                // should be happening in another task, nothing more to
                // do here.
            }
        }
    }
}

pub async fn user_disconnected(user_id: &str, conn: &Connections) {
    eprintln!("good bye user: {}", user_id);

    // Stream closed up, so remove from the user list
    conn.lock().unwrap().remove(user_id);
}
