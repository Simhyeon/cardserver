use warp::Filter;

use crate::handlers::*;

pub fn create_room(conn: &Connections) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("create")
        .and(warp::ws())
        .and(with_conns(conn.clone()))
        .and_then(create_handler)
}

pub fn join_room(conn: &Connections) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("join")
        .and(warp::ws())
        .and(warp::path::param())
        .and(with_conns(conn.clone()))
        .and_then(join_handler)
}
