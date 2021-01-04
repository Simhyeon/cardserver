use crate::models;

#[test]
fn function_name_test() {
    //let mut new_pool = models::CardPool::new();

    //// TEST :: Print card pool
    ////for item in new_pool.cards {
        ////println!("{:?}", item);
    ////}
    ////
    //// TEST :: Pool card and check if card was successfully deleted.
    //println!("{:?}", new_pool.cards.len());
    //let new_card = new_pool.poll_card();

    //println!("{:?}", new_card);
    //println!("{:?}", new_pool.cards.len());


    // TEST ::: Serde_Json for option
    let result = serde_json::to_string(&models::UserRequest{action: models::PlayerAction::MESSAGE, value: Some(25)});

    if let Ok(content) = result {
        println!("{:?}", content);
    }
}
