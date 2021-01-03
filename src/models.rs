use serde::{ Deserialize , Serialize};
use tokio::sync::mpsc;
use warp::ws::Message;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use rand::prelude::*;

const CARD_NUMBER : usize = 14;

pub struct Connection {
    pub creator_id: String,
    pub room_id: String,
    pub participant_id: Option<String>,
    pub sender : mpsc::UnboundedSender<Result<Message, warp::Error>>,
    pub game: Game,
}

impl Connection {
    pub fn new(
        creator_id: String, 
        room_id: String, 
        sender: mpsc::UnboundedSender<Result<Message, warp::Error>>
        ) -> Self {
        Self {  
            creator_id,
            room_id,
            participant_id: None,
            sender,
            game: Game::new(),
        }
    }
}

pub struct Game {
    pub turn: Turn,
    // TODO :: Refactor to use playerstat struct.
    pub creator_cards: Vec<Card>,
    pub participant_cards: Vec<Card>,

    pub card_pool : CardPool,
}

// Game related logics
impl Game {
    pub fn new() -> Self {
        // TODO :: Should poll cards several times.
        // before starting game.
        Self {  
            turn: Turn::CREATOR,
            creator_cards: vec![],
            participant_cards: vec![],
            card_pool: CardPool::new(),
        }
    }

    pub fn receive_player_action(&mut self, turn: Turn, action: PlayerAction) {
        // TODO :: Make it work
        // Calculate according to given player action.
        // Validate action if not then demand action again.
        match action {
            PlayerAction::POLLCARD => {
                if let Some(card) = self.card_pool.poll_card() {
                    // Creator's turn
                    if let Turn::CREATOR = turn {
                        self.creator_cards.push(card);
                    }
                    // participant's turn
                    else {
                        self.participant_cards.push(card);
                    }
                } else {
                    eprintln!("Failed poll card from card pool");
                }
            }
            _ => {}
        }
    }

    fn get_creator_cards_combination(&self) -> CardCombination {
        // TODO ::: Make it work
        CardCombination::NONE
    }

    fn get_participant_cards_combination(&self) -> CardCombination {
        // TODO ::: Make it work
        CardCombination::NONE
    }
}

pub enum Turn {
    CREATOR,
    PARTICIPANT,
}

pub struct PlayerStat {
    pub cash : u32,
    pub bet : u32,
    pub cards: Vec<Card>,
}

pub struct CardPool {
    pub cards : Vec<Card>,
    rng: ThreadRng,
}

impl CardPool {
    pub fn new() -> Self {
        let rng = thread_rng();
        let mut cards: Vec<Card> = vec![];
        for card_type in CardType::iter() {
            for number in 0..CARD_NUMBER {
                cards.push(Card{card_type, number : number as u8})
            }
        }
        Self {  
            cards,
            rng,
        }
    }

    pub fn poll_card(&mut self) -> Option<Card> {
        if self.cards.len() == 0 {return None;}

        let index = self.rng.gen_range(0..self.cards.len());

        Some(self.cards.remove(index))
    }
}

#[derive(Debug)]
pub struct Card {
    card_type: CardType,
    number: u8,
}

#[derive(Debug ,Clone, Copy, EnumIter)]
pub enum CardType {
    DIAMOND,
    SPADE,
    HEART,
    CLOVER,
}

pub enum CardCombination {
    NONE,
    FULLHOUSE,
    DOUBLE,
    TRIPLE,
    STRAIGHT,
    FLUSH,
    SFLUSH,
    RSFLUSH,
}

#[derive(Serialize, Deserialize)]
pub enum PlayerAction {
    POLLCARD,
    BETRAISE,
    BETCALL,
    FOLD,
}
