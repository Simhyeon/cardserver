mod models;
mod handlers;
mod test;

use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use warp::Filter;

use crate::handlers::*;

#[tokio::main]
async fn main() {
    // TODO ::: Refacotr Connections from type alias to proper struct 
    let conn = Connections::new(Mutex::new(HashMap::new()));
    let ws = warp::path("echo")
        // The `ws()` filter will prepare the Websocket handshake.
        .and(warp::ws())
        .and(with_conns(conn.clone()))
        .and_then(ws_handler);

    let routes = ws.with(warp::cors());

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
