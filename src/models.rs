use std::time::Duration;
use serde::{ Deserialize , Serialize};
use tokio::sync::mpsc;
use warp::ws::Message;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use rand::prelude::*;
use uuid::Uuid;

const BET_TIME : u64 = 15;
const CARD_NUMBER : usize = 14;
const DEFAULT_HP : u32 = 30;

// TODO :: Make submodels

// TODO :: Actually single Connection hashmap is really inefficient.
// Rather make it an array of multiple hashamp. 
// Or implement multi refernece approcach.
pub struct Connection {
    pub room_id: String,
    pub game: Game,
}

impl Connection {
    pub fn new(
        creator_id: String, 
        room_id: String, 
        sender: mpsc::UnboundedSender<Result<Message, warp::Error>>,
        internal_sender: mpsc::UnboundedSender<Result<Message, warp::Error>>
    ) -> Self {
        Self {  
            room_id,
            game: Game::new(creator_id, sender, internal_sender),
        }
    }
}

pub struct Game {
    pub state: GameState,
    pub state_id: Option<String>,
    pub state_extend: bool,
    pub internal_sender: mpsc::UnboundedSender<Result<Message, warp::Error>>,
    pub creator: User,
    pub participant: Option<User>,
    pub community: Vec<Card>,
    pub card_pool : CardPool,
}

// Game related logics
impl Game {
    pub fn new(
        cid: String, 
        sender: mpsc::UnboundedSender<Result<Message, warp::Error>>,
        internal_sender: mpsc::UnboundedSender<Result<Message, warp::Error>>,
    ) -> Self {
        // TODO :: Should poll cards several times.
        // before starting game.
        Self {  
            state: GameState::Flop,
            state_id: None,
            state_extend: false,
            internal_sender,
            creator: User::new(cid, sender),
            participant: None,
            community: vec![],
            card_pool: CardPool::new(),
        }
    }

    pub fn set_state_id_and_send(&mut self) {
        // Set State id and send messages to each clients.
        self.state_id.replace(
            Uuid::new_v4().to_simple().to_string()
        );

        let res_state = ServerResponse::new_json(
            ResponseType::State,
            ResponseValue::State((self.state ,self.state_id.as_ref().unwrap().clone()))
        ).expect("Failed to create resonse");
        self.creator.send_message(&res_state);
        self.participant.as_ref().unwrap().send_message(&res_state);
    }

    pub fn init_game(&mut self) {
        if let None = self.participant {
            eprintln!("Tried to init a game with no participant.");
            return;
        }

        // NOTE
        // This can theoritically fail 
        // However card pool is always re initialized every round
        // So in intended scenario, it never fails.
        self.community = self.card_pool.poll_cards(3).unwrap();
        self.creator.stat.cards = self.card_pool.poll_cards(2).unwrap();
        self.participant.as_mut().unwrap().stat.cards = self.card_pool.poll_cards(2).unwrap();

        let res_community = ServerResponse::new_json(
            ResponseType::Community, 
            ResponseValue::Card(self.community.clone())
        ).expect("Failed to create server response");
        self.creator.send_message(&res_community);
        self.participant.as_ref().unwrap().send_message(&res_community);

        let res_creator = ServerResponse::new_json(
            ResponseType::Hand, 
            ResponseValue::Card(self.creator.stat.cards.clone())
        ).expect("Failed to create server response");
        self.creator.send_message(&res_creator);

        let res_part = ServerResponse::new_json(
            ResponseType::Hand, 
            ResponseValue::Card(self.participant.as_ref().unwrap().stat.cards.clone())
        ).expect("Failed to create server response");
        self.participant.as_ref().unwrap().send_message(&res_part);

        self.set_state_id_and_send();

        // Set timeout by sending request through internal channel
        let req_timeout =InternalRequest::new_json(IntReqType::TimeOut, IntReqValue::Duration(BET_TIME))
            .expect("Failed to create internal request");
        let result = self.internal_sender.send(Ok(Message::text(req_timeout)));
        match result {
            Ok(()) => {
                eprintln!("Successfully sent internal request");
            }
            Err(_) => {
                eprintln!("Couldn't send internal request");
            }
        }
        // TODO Delete this line
        // This is for reference.
        //self.internal_sender.send(Ok(Message::text(req_timeout)))
            //.expect("Failed to send internal request");
    }

    pub fn broadcast_message(&self, msg: &str) {
        if let None = self.participant {
            return;
        }

        self.participant.as_ref().unwrap().send_message(msg);
        self.creator.send_message(msg);
    }

    pub fn next_state(&mut self) {
        match self.state {
            GameState::Flop => {
                self.state = GameState::Turn;
            }
            GameState::Turn => {
                self.state = GameState::River;
            }
            GameState::River | GameState::Fold => {
                self.state = GameState::ShowDown;
            }
            GameState::ShowDown => {
                self.state = GameState::Flop;
            }
        }

        self.set_state_id_and_send();
    }

    pub fn pending_next_state(&mut self, pending: Pending) {
        if let Pending(Some(state)) = pending {
            eprintln!("It's pending, step to next state");
            match state {
                GameState::Flop => {
                    self.state = GameState::Turn;
                }
                GameState::Turn => {
                    self.state = GameState::River;
                }
                GameState::River | GameState::Fold => {
                    self.state = GameState::ShowDown;
                }
                GameState::ShowDown => {
                    self.state = GameState::Flop;
                }
            }

            // TODO 
            // Do something necessary for initialization

            self.set_state_id_and_send();
        }
    }

    pub fn receive_player_action(&mut self, uid: &str, req: UserRequest) -> Pending {

        // If state is different from current state,
        // It means request is outdated or modified.
        if &req.state_id != self.state_id.as_ref().unwrap() {
            return Pending(None);
        }

        // If room is not complete, return
        if let None = self.participant {
            eprintln!("Tried to retrive action while room is not complete");
            return Pending(None);
        }
        
        let user: &mut User;
        let opp: &mut User;

        if uid == self.creator.id {
            user = &mut self.creator;
            opp = self.participant.as_mut().unwrap();
        } else {
            user = self.participant.as_mut().unwrap();
            opp = &mut self.creator;
        }

        let mut pending = Pending(None);

        // TODO :: Make it work
        // Calculate according to given player action.
        // Validate action if not then demand action again.
        match req.action {
            PlayerAction::Fold => {
                user.fold();
            }
            PlayerAction::Message => {
                if uid == user.id {
                    opp.send_message("Ping from opponent");
                }
                // participant's turn
                else {
                    user.send_message("Ping from opponent");
                }
            }
            // For Check, Raise, Call(Raise)
            _ => {
                if let Some(amount) = req.value {
                    user.bet(amount);

                    if let PlayerAction::Raise = req.action {
                        opp.send_message(
                            &ServerResponse::new_json(
                                ResponseType::Raise, 
                                ResponseValue::Raise(amount)
                            ).expect("Failed to create server response"));

                        // And lengthen timeout period.
                        self.state_extend = true;
                    }
                } else {
                    eprintln!("Invalid syntax");
                }

            }
        }

        if let PlayerAction::Message = req.action{}
        else {
            user.current_action = req.action;


            // TODO :: Check if server can change the state 
            // thus make Pending current state.
            // if all players' have bet.
            // change the state.
            if user.current_action == opp.current_action ||
                user.current_action == PlayerAction::Fold ||
                    opp.current_action == PlayerAction::Fold {
                        eprintln!("PENDING!");
                        pending = Pending(Some(self.state));
            }
        }

        pending
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
    pub current_action: PlayerAction,
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
            current_action: PlayerAction::None,
            sender,
            stat: PlayerStat::new(),
        }
    }

    // TODO
    pub fn get_card_combination(&self) -> CardCombination {
        CardCombination::HighCard
    }

    pub fn add_card(&mut self, card: Card) {
        self.stat.cards.push(card);
    }

    pub fn bet(&mut self, amount: u32) {
        if let Some(value) = self.stat.bet {
            self.stat.bet.replace(value+amount);
        } else {
            self.stat.bet.replace(amount);
        }
    }

    pub fn fold(&mut self) {
        self.stat.bet = None;
    }

    pub fn send_message(&self, msg :&str) {
        self.sender.send(Ok(Message::text(msg)))
            .expect("Failed to send message");
    }
}

pub struct PlayerStat {
    pub hp: u32,
    pub bet : Option<u32>,
    pub cards: Vec<Card>,
}

impl PlayerStat {
    pub fn new() -> Self {
        Self {  
            hp: DEFAULT_HP,
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

    pub fn poll_cards(&mut self, count: usize) -> Option<Vec<Card>> {
        if self.cards.len() == 0 || self.cards.len() < count {return None;}

        let mut cards = vec![];

        // TODO ::: 
        // This is not necessarily a great optimization since creation of thread local
        // generator is not lightoperation. 
        for _ in 0..count {
            let index = rand::thread_rng().gen_range(0..self.cards.len());
            cards.push( self.cards.remove(index) );
        }

        Some(cards)
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
    HighCard,
    Pair,
    TwoPair,
    ThreeOfaKind,
    FullHouse,
    Straight,
    Flush,
    Sflush,
    Rflush,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum PlayerAction {
    None,
    Message,
    Fold,
    Check,
    Raise,
    // Call, -> Call is conceptual and it is processed as if raise
}

#[derive(Serialize, Deserialize)]
pub struct UserRequest {
    pub state_id: String,
    pub action: PlayerAction,
    pub value: Option<u32>,
}

impl UserRequest{
    pub fn dummy() -> Self {
        Self {
            state_id: "".to_string(),
            action: PlayerAction::None,
            value: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum ResponseType {
    State,
    Community,
    Hand,
    Message,
    Raise,
}

#[derive(Serialize, Deserialize)]
pub struct ServerResponse {
    pub response_type: ResponseType,
    pub value: ResponseValue,
}

impl ServerResponse{
    pub fn new_json(response_type: ResponseType, value: ResponseValue) -> Result<String, serde_json::Error> {
        serde_json::to_string(&ServerResponse {
            response_type,
            value
        })
    }
}

#[derive(Serialize, Deserialize)]
pub enum ResponseValue {
    State(( GameState , String)),
    Message(String),
    Card(Vec<Card>),
    Raise(u32),
}

pub struct Pending(Option<GameState>);

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum GameState {
    Flop,
    Turn,
    River,
    ShowDown,
    Fold,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InternalRequest {
    pub request_type :IntReqType,
    pub value: IntReqValue,
}

impl InternalRequest {
    pub fn dummy() -> Self {
        Self {
            request_type: IntReqType::None,
            value: IntReqValue::None,
        }
    }

    pub fn new_json(request_type: IntReqType, value: IntReqValue) -> Result<String, serde_json::Error> {
        serde_json::to_string(&InternalRequest {
            request_type,
            value
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum IntReqType {
    None,
    Message,
    TimeOut,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum IntReqValue {
    None,
    Message(String),
    Duration(u64),
}
