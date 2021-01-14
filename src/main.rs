mod models;
mod handlers;
mod routes;
mod test;

use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use warp::Filter;

use crate::handlers::*;
use crate::routes::*;

#[tokio::main]
async fn main() {
    // TODO ::: Refacotr Connections from type alias to proper struct 
    let conn = Connections::new(RwLock::new(HashMap::new()));

    let create_room = routes::create_room(&conn);
    //let get_rooms_list = routes::get_room();
    let join_room = routes::join_room(&conn);

    let routes = create_room
        //.or(get_room)
        .or(join_room)
        .with(warp::cors());

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
