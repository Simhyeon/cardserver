use crate::models::{CardPool, Card, CardType, CombinationBuilder};
use rand::prelude::*;
use rand::distributions::{Distribution, Uniform};

#[test]
fn function_name_test() {
    //manual_comb_test();
    rand_comb_test();
    //

    //let mut array = vec![];
    //let mut array_from = vec![1,2,3,4,5,6,7,8,9,10];
    //let mut rng = rand::thread_rng();
    //// TODO ::: 
    //// This is not necessarily a great optimization since creation of thread local
    //// generator is not lightoperation. 
    //for _ in 0..3 {
        //let between = Uniform::from(0..array_from.len());
        //let index = between.sample(&mut rng);
        ////let index = rand::thread_rng().gen_range(1..self.cards.len());
        //array.push(array_from.remove(index));
    //}

    //println!("{:?}", array);
}

fn manual_comb_test() {
    let mut cards: Vec<Card> = vec![
        Card::new(CardType::Spade, 10),
        Card::new(CardType::Diamond, 11),
        Card::new(CardType::Heart, 12),
        Card::new(CardType::Spade, 9),
        Card::new(CardType::Spade, 8),
        Card::new(CardType::Diamond, 7),
    ];

    let mut printer = cards.clone();
    printer.sort_by(|a,b| a.number.cmp(&b.number));

    for item in printer {
        println!("{:?}", item);
    }

    let (highest, meta) = CombinationBuilder::get_highest_combination(cards);
    println!("Highest combination is : {:?}, meta : {:?}", highest, meta);
}

fn rand_comb_test() {
    let mut card_pool = CardPool::new();
    let mut cards = vec![];
    let mut cards_2 = vec![];
    for _ in 0..4 {
        let index = rand::thread_rng().gen_range(0..card_pool.cards.len());
        cards.push( card_pool.cards.remove(index) );
    }

    for _ in 0..2 {
        let index = rand::thread_rng().gen_range(0..card_pool.cards.len());
        cards.push( card_pool.cards.remove(index) );
    }

    let mut merged = cards.iter().chain(cards_2.iter()).cloned().collect::<Vec<Card>>();

    let mut printer = merged.clone();
    printer.sort_by(|a,b| a.number.cmp(&b.number));

    for item in printer {
        println!("{:?}", item);
    }

    let ( highest , meta) = CombinationBuilder::get_highest_combination(merged);
    println!("Highest combination is : {:?}, meta :{:?}", highest, meta);
}
