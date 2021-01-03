use serde::{ Deserialize , Serialize};
use tokio::sync::mpsc;
use warp::ws::Message;

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
    pub creator_cards: Vec<Card>,
    pub participant_cards: Vec<Card>,
}

impl Game {
    pub fn new() -> Self {
        Self {  
            turn: Turn::CREATOR,
            creator_cards: vec![],
            participant_cards: vec![],
        }
    }

    pub fn receive_player_action(action:PlayerAction) {
        // TODO :: Make it work
        // Calculate according to given player action.
        // Validate action if not then demand action again.
        match action {
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

pub struct Card {
    card_type: CardType,
    number: u8,
}

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
    TRIPEL,
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
    BETFOLD,
}
