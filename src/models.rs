use serde::{ Deserialize , Serialize};
use tokio::sync::mpsc;
use warp::ws::Message;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use rand::prelude::*;

const CARD_NUMBER : usize = 14;

pub struct Connection {
    pub room_id: String,
    pub game: Game,
}

impl Connection {
    pub fn new(
        creator_id: String, 
        room_id: String, 
        sender: mpsc::UnboundedSender<Result<Message, warp::Error>>
    ) -> Self {
        Self {  
            room_id,
            game: Game::new(creator_id, sender),
        }
    }
}

pub struct Game {
    pub turn: Turn,
    // TODO :: Refactor to use playerstat struct.
    pub creator: User,
    pub participant: Option<User>,
    pub card_pool : CardPool,
}

// Game related logics
impl Game {
    pub fn new(
        cid: String, 
        sender: mpsc::UnboundedSender<Result<Message, warp::Error>>
    ) -> Self {
        // TODO :: Should poll cards several times.
        // before starting game.
        Self {  
            turn: Turn::CREATOR,
            creator: User::new(cid, sender),
            participant: None,
            card_pool: CardPool::new(),
        }
    }

    pub fn send_message(&self, user_id: String, msg: Message) {
        if let None = self.participant {
            return;
        }

        // From creator to participant
        if user_id == self.creator.id {
            self.participant.as_ref().unwrap().sender.send(Ok(msg))
                .expect("Failed to send message");
        } 
        // From participant to creator
        else {
            self.creator.sender.send(Ok(msg))
                .expect("Failed to send message");
        }
    }

    pub fn receive_player_action(&mut self, uid: &str, action: PlayerAction) {

        if let None = self.participant {
            eprintln!("Tried to retrive action while room is not complete");
            return;
        }

        // TODO :: Make it work
        // Calculate according to given player action.
        // Validate action if not then demand action again.
        match action {
            PlayerAction::Pollcard => {
                // TODO :: I'm not sure if creating json object from card array is appropriate
                // or just try to make json object from simple array.
                if let Some(card) = self.card_pool.poll_card() {
                    eprintln!("Successfully polled card from cardpool");
                    // Creator's turn
                    if uid == self.creator.id {
                        self.creator.add_card(card.clone());
                        let res = serde_json::to_string(&ServerResponse{response_type: ResponseType::Card, value : ResponseValue::Card(card)})
                            .expect("Failed to create json response");
                        self.creator.sender.send( Ok(Message::text(res)) )
                            .expect("Failed to send response");
                    }
                    // participant's turn
                    else {
                        self.participant.as_mut().unwrap().add_card(card.clone());
                        let cards = serde_json::to_string(&self.participant.as_ref().unwrap().stat.cards)
                            .expect("Failed to created cards json objects");
                        let res = serde_json::to_string(&ServerResponse{response_type: ResponseType::Card, value : ResponseValue::Card(card)})
                            .expect("Failed to create json response");
                        self.participant.as_ref().unwrap().sender.send( Ok(Message::text(res)) )
                            .expect("Failed to send response");
                    }
                } else {
                    eprintln!("Failed to poll card from card pool");
                }
            }
            PlayerAction::Message => {
                if uid == self.creator.id {

                    self.participant.as_ref().unwrap().sender.send(Ok(Message::text("Ping from opponent")))
                        .expect("Failed to send message");
                }
                // participant's turn
                else {
                    self.creator.sender.send(Ok(Message::text("Ping from opponent")))
                        .expect("Failed to send message");
                }
            }
            _ => {
                eprintln!("Action not found which is {:?}", action);
            }
        }
    }

    pub fn join_game(
        &mut self,
        id: String, 
        sender: mpsc::UnboundedSender<Result<Message, warp::Error>>
    ) {
        self.participant.replace(User::new(id, sender));

        // TODO Start a game.
    }
}

pub struct User {
    pub id : String,
    pub sender : mpsc::UnboundedSender<Result<Message, warp::Error>>,
    pub stat: PlayerStat,
}

impl User {
    pub fn new(
        id: String, 
        sender: mpsc::UnboundedSender<Result<Message, warp::Error>>,
    ) -> Self {
        Self {  
            id,
            sender,
            stat: PlayerStat::new(),
        }
    }

    // TODO
    pub fn get_card_combination(&self) -> CardCombination {
        CardCombination::None
    }

    pub fn add_card(&mut self, card: Card) {
        self.stat.cards.push(card);
    }

    pub fn bet(&mut self, amount: u32) {
        self.stat.bet.replace(amount);
    }
}

pub struct PlayerStat {
    pub cash : u32,
    pub bet : Option<u32>,
    pub cards: Vec<Card>,
}

pub enum Turn {
    CREATOR,
    PARTICIPANT,
}

impl PlayerStat {
    pub fn new() -> Self {
        Self {  
            cash: 0,
            bet: None,
            cards: vec![],
        }
    }
}

pub struct CardPool {
    pub cards : Vec<Card>,
}

impl CardPool {
    pub fn new() -> Self {
        let mut cards: Vec<Card> = vec![];
        for card_type in CardType::iter() {
            for number in 0..CARD_NUMBER {
                cards.push(Card{card_type, number : number as u8})
            }
        }
        Self {  
            cards,
        }
    }

    pub fn poll_card(&mut self) -> Option<Card> {
        if self.cards.len() == 0 {return None;}

        // TODO ::: 
        // This is not necessarily a great optimization since creation of thread local
        // generator is not lightoperation. 
        let index = rand::thread_rng().gen_range(0..self.cards.len());

        Some(self.cards.remove(index))
    }
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct Card {
    card_type: CardType,
    number: u8,
}

#[derive(Debug ,Clone, Copy, EnumIter, Serialize, Deserialize)]
pub enum CardType {
    Diamond,
    Spade,
    Heart,
    Clover,
}

pub enum CardCombination {
    None,
    Fullhouse,
    Double,
    Triple,
    Straight,
    Flush,
    Sflush,
    Rsflush,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PlayerAction {
    Message,
    Poll_Card,
    Bet_Raise,
    Bet_Call,
    Fold,
}

#[derive(Serialize, Deserialize)]
pub enum ResponseType {
    Card,
    Message,
}

#[derive(Serialize, Deserialize)]
pub struct UserRequest {
    pub action: PlayerAction,
    pub value: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct ServerResponse {
    pub response_type: ResponseType,
    pub value: ResponseValue,
}

#[derive(Serialize, Deserialize)]
pub enum ResponseValue {
    Message(String),
    Card(Card),
    Bet(u32),
}
