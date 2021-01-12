use std::collections::HashMap;
use std::hash::Hash;
use std::cmp::Ordering;
use strum_macros::Display;
use serde::{ Deserialize , Serialize};
use tokio::sync::mpsc;
use warp::ws::Message;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use rand::distributions::{Distribution, Uniform};
use rand::prelude::*;
use uuid::Uuid;

const CARD_MAX_NUMBER: usize = 13;
const COMB_COUNT: usize = 5;
const BET_TIME : u64 = 60;
const SHOWDOWN_TIME: u64 = 8;
const CARD_NUMBER : usize = 14;
const DEFAULT_HP : u32 = 20;

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

        // Create Timeout
        let req_timeout =InternalRequest::new_json(
            IntReqType::TimeOut, 
            IntReqValue::TimeOut(TimeOut{duration: std::time::Duration::from_secs(BET_TIME), state_id: self.state_id.as_ref().unwrap().clone()})
        ).expect("Failed to create internal request");
        if let Err(err) = self.internal_sender.send(Ok(Message::text(req_timeout))) {
            eprintln!("Couldn't send internal request \n {}", err);
        };
    }

    pub fn init_game(&mut self) {
        if let None = self.participant {
            eprintln!("Tried to init a game with no participant.");
            return;
        }
        self.send_env_variables();
        self.init_cards_and_send();
        self.set_state_id_and_send();

        // Set timeout by sending request through internal channel
        let req_timeout =InternalRequest::new_json(
            IntReqType::TimeOut, 
            IntReqValue::TimeOut(TimeOut{duration: std::time::Duration::from_secs(BET_TIME), state_id: self.state_id.as_ref().unwrap().clone()})
        ).expect("Failed to create internal request");
        if let Err(err) = self.internal_sender.send(Ok(Message::text(req_timeout))) {
            eprintln!("Couldn't send internal request : {}", err);
        }
    }

    fn send_env_variables(&self) {
        if let None = self.participant {
            eprintln!("Invalid operation participant is empty");
            return;
        }

        let res = ServerResponse::new_json(
            ResponseType::Env, 
            ResponseValue::Env(
                EnvVar{
                    hp: DEFAULT_HP,
                    bet_time: BET_TIME,
                    result_time: SHOWDOWN_TIME,
                }, 
            )
        ).expect("Failed to create server response");
        self.creator.send_message(&res);
        self.participant.as_ref().unwrap().send_message(&res);
    }

    fn init_cards_and_send(&mut self) {
        // Refresh card_pool
        self.card_pool = CardPool::new();

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
    }

    pub fn broadcast_message(&self, msg: &str) {
        if let None = self.participant {
            return;
        }

        self.participant.as_ref().unwrap().send_message(msg);
        self.creator.send_message(msg);
    }

    pub fn next_state(&mut self, state_id : &str) {
        // This should work in normal cases.
        // However it might be used in not desired
        // situations. Then error handing should be properly 
        // implemented.
        if self.state_id.as_ref().unwrap() != state_id {
            eprintln!("State id not equal, ignoring request");
            return;
        }
        //eprintln!("It's pending, step to next state");
        self.no_response_check();
        self.change_state(self.state);
    }

    fn no_response_check(&mut self) {
        let user = &mut self.creator;
        let opp = self.participant.as_mut().unwrap();

        if let PlayerAction::None = user.current_action {
            user.current_action = PlayerAction::Check;
        }

        if let PlayerAction::None = opp.current_action {
            opp.current_action = PlayerAction::Check;
        }
    }

    pub fn pending_next_state(&mut self, pending: Pending) {
        if let Pending(Some(state)) = pending {
            //eprintln!("It's pending, step to next state");
            self.change_state(state);
        }
    }

    fn change_state(&mut self, current_state: GameState) {
        //eprintln!("Change state");
        let mut new_card :Option<Card> = None;
        match current_state {
            GameState::Flop => {
                self.state = GameState::Turn;
                new_card.replace(self.add_community());
            }
            GameState::Turn => {
                self.state = GameState::River;
                new_card.replace(self.add_community());
            }
            GameState::River | GameState::Fold => {
                eprintln!("SHOWDOWN!");
                self.state = GameState::ShowDown;
                self.calculate_showdown();
            }
            GameState::ShowDown => {
                self.state = GameState::Flop;
            }
        }

        // TODO 
        // Do something necessary for initialization
        self.clear_user_action();
        self.set_state_id_and_send();

        if let Some(card) = new_card {
            let res = ServerResponse::new_json(ResponseType::Community, ResponseValue::Card(vec![card])).expect("Failed to create server response");
            self.creator.send_message(&res);
            self.participant.as_ref().unwrap().send_message(&res);
        }

        if let GameState::ShowDown = self.state {
            // Send Timeout request
            let req = InternalRequest::new_json(
                IntReqType::TimeOut, 
                IntReqValue::TimeOut(TimeOut {
                    duration: std::time::Duration::from_secs(SHOWDOWN_TIME), 
                    state_id: self.state_id.clone().unwrap()
                })).expect("Failed to create internal request");
            self.internal_sender.send(Ok(Message::text(req)))
                .expect("Failed to send internal request");
        } else if let GameState::Flop = self.state {
            self.clear_user_bet();
            self.init_cards_and_send();
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
                opp.current_action = PlayerAction::Check;
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

                        user.send_message(
                            &ServerResponse::new_json(
                                ResponseType::Delay, 
                                ResponseValue::Number(BET_TIME as i32)
                            ).expect("Failed to create server response"));

                        // And lengthen timeout period.
                        // Ths enables tokio tasks that wait for delay, can
                        // halt their action after the delay.
                        self.state_extend = true;

                        // TODO IMPORTANT
                        // Should this code go to next_state function ?
                        let req_timeout =InternalRequest::new_json(
                            IntReqType::TimeOut, 
                            IntReqValue::TimeOut(TimeOut{duration: std::time::Duration::from_secs(BET_TIME), state_id: self.state_id.as_ref().unwrap().clone()})
                        ).expect("Failed to create internal request");
                        let result = self.internal_sender.send(Ok(Message::text(req_timeout)));
                        match result {
                            Ok(()) => {
                                eprintln!("Successfully sent internal request");
                            }
                            Err(_) => {
                                eprintln!("Couldn't send internal request");
                            }
                        }
                    }
                } else {
                    eprintln!("Invalid syntax");
                }

            }
        }

        if let PlayerAction::Message = req.action{}
        else {
            if req.action == PlayerAction::Call {
                user.current_action = PlayerAction::Raise;
            } else {
                user.current_action = req.action;
            }


            let mut bet_end = false;
            // TODO :: Check if server can change the state 
            // thus make Pending current state.
            // if all players' have bet.
            // change the state.
            // Currently it reverts  bet by hard code if it gets non limit hold'em
            // it get's different and should be re-implemented.
            if user.current_action == opp.current_action {
                bet_end = true;
                pending = Pending(Some(self.state));
            } else if user.current_action == PlayerAction::Fold {
                bet_end = true;
                if let PlayerAction::Raise = opp.current_action {
                    opp.stat.bet -= 1;
                }
                pending = Pending(Some(GameState::Fold));
            } else if opp.current_action == PlayerAction::Fold {
                bet_end = true;
                if let PlayerAction::Raise = user.current_action {
                    user.stat.bet -= 1;
                }
                pending = Pending(Some(GameState::Fold));
            }

            if bet_end {
                self.end_bet();
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
    }

    fn end_bet(&self) {

        if let None = self.participant {
            return;
        }

        let total_bet = self.get_total_bet();
        self.creator.send_message(
            &ServerResponse::new_json(
                ResponseType::BetResult, 
                ResponseValue::BetResult(BetResult{opponent_action: self.participant.as_ref().unwrap().current_action, total_bet})
            ).expect("Failed to create server response")
        );

        self.participant.as_ref().unwrap().send_message(
            &ServerResponse::new_json(
                ResponseType::BetResult, 
                ResponseValue::BetResult(BetResult{opponent_action: self.creator.current_action, total_bet})
            ).expect("Failed to create server response")
        );
    }

    // Prefere this method rather than manually adding two bets
    fn get_total_bet(&self) -> u32 {
        // NOTE Hard coded initial bet which is 2 in this case.
        // Due to mutural refernce rule total_bet should be boxed into variable
        self.participant.as_ref().unwrap().stat.bet + self.creator.stat.bet + 2
    }

    fn add_community(&mut self) -> Card {
        if let Some(card) = self.card_pool.poll_card() {
            self.community.push(card.clone());
            card
        } else {
            panic!("This should not happen. This error occured because every possible card in card pools has been polled");
        }
    }

    fn clear_user_bet(&mut self) {
        if let None =self.participant {
            eprintln!("Invalid work flow should call function clear_user_bet when participant is not empty");
            return;
        }

        self.creator.stat.bet = 0;
        self.creator.stat.fold = false;
        self.participant.as_mut().unwrap().stat.bet = 0;
        self.participant.as_mut().unwrap().stat.fold = false;
    }
    fn clear_user_action(&mut self) {
        if let None =self.participant {
            eprintln!("Invalid work flow should call function clear_user_action when participant is not empty");
            return;
        }
        self.creator.current_action = PlayerAction::None;
        self.participant.as_mut().unwrap().current_action = PlayerAction::None;
    }

    fn calculate_showdown(&mut self) {

        let mut cmp_result : Ordering = Ordering::Equal;

        // Check if player has folded or not
        if self.creator.stat.fold {
            cmp_result = Ordering::Less;
        } else if self.participant.as_ref().unwrap().stat.fold {
            cmp_result = Ordering::Greater;
        }

        let user_iter = self.community.iter().chain(self.creator.stat.cards.iter());
        let participant_iter = 
            self.community.iter().chain(self.participant.as_ref().unwrap().stat.cards.iter());

        // Create whole card iterator from two iterator 
        let user_card_array = user_iter.cloned().collect::<Vec<Card>>();
        let part_card_array = participant_iter.cloned().collect::<Vec<Card>>();

        // User_meat mostly means number of high card.
        let ( user_comb , user_meta ) = 
            CombinationBuilder::get_highest_combination(user_card_array);
        let ( part_comb , part_meta ) = 
            CombinationBuilder::get_highest_combination(part_card_array);

        // If ordering is set either greater or less,
        // it means someone has folded.
        // If no fold is found then calculate normally.
        // else don't do comparison
        if let Ordering::Equal = cmp_result {
            cmp_result = (user_comb as u8).cmp(&(part_comb as u8));
        }

        if let Ordering::Equal =  cmp_result.clone() {
            // if both player is high card,
            // compare both numbers.
            // and set comparison again.
            if let Some(number) = user_meta {
                let user_number = number.parse::<i32>().unwrap_or(0);
                let part_number = part_meta.unwrap_or("0".to_string()).parse::<i32>().unwrap_or(0);

                let meta_result = user_number.cmp(&part_number).reverse();
                match meta_result {
                    // user wins
                    Ordering::Greater => {
                        cmp_result = Ordering::Greater;
                    }
                    // participant wins
                    Ordering::Less => {
                        cmp_result = Ordering::Less;
                    }
                    // draws or both is high number
                    Ordering::Equal => {}
                }
            }
        }

        // Cached participant user struct
        match cmp_result {
            Ordering::Greater => {
                // Calculate damage 
                // Cache required to comply with single reference rule
                let total_bet  = self.get_total_bet();
                self.participant.as_mut().unwrap().apply_damage(total_bet);
            }
            Ordering::Less => {
                // Calculate damage 
                self.creator.apply_damage(self.get_total_bet());
            }
            Ordering::Equal => {}
        }
        self.send_showdown_result(cmp_result, user_comb, part_comb)
    }

    fn send_showdown_result(&mut self, comparison: Ordering, user_comb: CardCombination, part_comb: CardCombination) {
        let mut user_win_check : Option<bool> = None;
        let mut opp_win_check : Option<bool> = None;
        match comparison {
            Ordering::Greater => {
                user_win_check = Some(true);
                opp_win_check = Some(false);
            },
            Ordering::Less => {
                user_win_check = Some(false);
                opp_win_check = Some(true);
            },
            _ => {}
        }
        let to_creator_response = 
            ServerResponse::new_json(
                ResponseType::RoundResult, 
                ResponseValue::RoundResult(RoundResult {
                    win: user_win_check,
                    fold: self.creator.stat.fold,
                    opp_fold: self.participant.as_ref().unwrap().stat.fold,
                    comb: user_comb,
                    opp_comb: part_comb,
                    hp: self.creator.stat.hp,
                    opp_hp: self.participant.as_ref().unwrap().stat.hp,
                })
            ).expect("Failed to create server reseponse");

        let to_part_response = 
            ServerResponse::new_json(
                ResponseType::RoundResult, 
                ResponseValue::RoundResult(RoundResult {
                    win: opp_win_check,
                    fold: self.participant.as_ref().unwrap().stat.fold,
                    opp_fold: self.creator.stat.fold,
                    comb: part_comb,
                    opp_comb: user_comb,
                    hp: self.participant.as_ref().unwrap().stat.hp,
                    opp_hp: self.creator.stat.hp,
                })
            ).expect("Failed to create server reseponse");

        self.creator.send_message(&to_creator_response);
        self.participant.as_ref().unwrap().send_message(&to_part_response);

        self.send_game_result();
    }

    fn send_game_result(&mut self) {
        let user_game_winner: bool;

        if self.creator.stat.hp == 0 {
            user_game_winner = true;
        } else if self.participant.as_ref().unwrap().stat.hp == 0 {
            user_game_winner = false;
        } else {
            return;
        }

        let to_creator_response = 
            ServerResponse::new_json(
                ResponseType::GameResult, 
                ResponseValue::GameResult(user_game_winner)
            ).expect("Failed to create server reseponse");

        let to_part_response = 
            ServerResponse::new_json(
                ResponseType::GameResult, 
                ResponseValue::GameResult(!user_game_winner)
            ).expect("Failed to create server reseponse");

        self.creator.send_message(&to_creator_response);
        self.participant.as_ref().unwrap().send_message(&to_part_response);

        self.end_game();
    }

    fn end_game(&mut self) {
        // change state id so that
        // timer doesn't called.
        self.state_id.replace(
            Uuid::new_v4().to_simple().to_string()
        );

        let req_timeout =InternalRequest::new_json(
            IntReqType::GameEnd,
            IntReqValue::None
        ).expect("Failed to create internal request");
        if let Err(err) = self.internal_sender.send(Ok(Message::text(req_timeout))) {
            eprintln!("Couldn't send internal request \n {}", err);
        };

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

    // Bet should be incremental
    pub fn bet(&mut self, amount: u32) {
        self.stat.bet += amount;
    }

    pub fn fold(&mut self) {
        self.stat.bet = 0;
        self.stat.fold = true;
    }

    pub fn send_message(&self, msg :&str) {
        if let Err(err) = self.sender.send(Ok(Message::text(msg))) {
            eprintln!("Failed to send message : \n {}", err);
        }
    }

    pub fn apply_damage(&mut self, damage: u32) {
        if self.stat.hp < damage { 
            self.stat.hp = 0; 
        } else {
            self.stat.hp -= damage;
        }
    }
}

pub struct PlayerStat {
    pub fold: bool,
    pub hp: u32,
    pub bet : u32,
    pub cards: Vec<Card>,
}

impl PlayerStat {
    pub fn new() -> Self {
        Self {  
            fold: false,
            hp: DEFAULT_HP,
            bet: 0,
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
            for number in 1..CARD_NUMBER {
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
        let index = rand::thread_rng().gen_range(1..self.cards.len());

        Some(self.cards.remove(index))
    }

    pub fn poll_cards(&mut self, count: usize) -> Option<Vec<Card>> {
        if self.cards.len() == 0 || self.cards.len() < count {return None;}

        let mut cards = vec![];

        let mut rng = rand::thread_rng();
        // TODO ::: 
        // This is not necessarily a great optimization since creation of thread local
        // generator is not lightoperation. 
        for _ in 0..count {
            let between = Uniform::from(0..self.cards.len());
            let index = between.sample(&mut rng);
            //let index = rand::thread_rng().gen_range(1..self.cards.len());
            cards.push( self.cards.remove(index) );
        }

        Some(cards)
    }
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, PartialOrd, Eq)]
pub struct Card {
    pub card_type: CardType,
    pub number: u8,
}

impl Card {
    pub fn new(card_type: CardType, number: u8) -> Self {
        Self {  
            card_type, 
            number,
        }
    }
}

impl Ord for Card {
    fn cmp(&self, other: &Self) -> Ordering {
        let type_cmp = self.card_type.to_string().cmp(&other.card_type.to_string());
        //if let Ordering::Equal = type_cmp {
            //self.number.cmp(&other.number)
        //} else {
            //type_cmp
        //}
        type_cmp
    }
}

#[derive(Debug ,Clone, Copy, EnumIter, Serialize, Deserialize, PartialEq, PartialOrd, Eq, Display, Hash)]
pub enum CardType {
    Diamond,
    Spade,
    Heart,
    Clover,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum CardCombination {
    HighCard = 0,
    Pair = 1,
    TwoPair = 2,
    ThreeOfaKind = 3,
    FullHouse = 4,
    Straight = 5 ,
    Flush = 6,
    Sflush = 7,
    Rflush = 8,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Copy, Clone)]
pub enum PlayerAction {
    None,
    Message,
    Fold,
    Check,
    Raise,
    Call, 
}

#[derive(Serialize, Deserialize, Debug)]
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
    Env,
    State,
    Community,
    Hand,
    Message,
    Raise,
    Delay,
    BetResult,
    RoundResult,
    GameResult,
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
    Env(EnvVar),
    State(( GameState , String)),
    BetResult(BetResult),
    RoundResult(RoundResult),
    GameResult(bool),
    Message(String),
    Card(Vec<Card>),
    Raise(u32),
    Number(i32),
}

pub struct Pending(Option<GameState>);

#[derive(Serialize, Deserialize)]
pub struct EnvVar {
    hp: u32,
    bet_time: u64,
    result_time: u64,
}

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
    GameEnd,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum IntReqValue {
    None,
    Message(String),
    TimeOut(TimeOut),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TimeOut {
    pub duration: std::time::Duration,
    pub state_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BetResult {
    pub opponent_action: PlayerAction,
    pub total_bet : u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RoundResult {
    pub win: Option<bool>,
    pub fold: bool,
    pub opp_fold: bool,
    pub comb: CardCombination,
    pub opp_comb: CardCombination,
    pub hp : u32,
    pub opp_hp : u32,
}

pub struct CombinationBuilder;

impl CombinationBuilder {
    pub fn get_highest_combination(mut cards : Vec<Card>) -> (CardCombination, Option<String>) {
        if cards.len() <= 1 {
            panic!("Invalid card vector given to function : get_highest_combination");
        }

        cards.sort_by(|a,b| a.number.cmp(&b.number));
        
        let mut type_map = std::collections::HashMap::new();
        type_map.insert(CardType::Heart, 0);
        type_map.insert(CardType::Clover, 0);
        type_map.insert(CardType::Spade, 0);
        type_map.insert(CardType::Diamond, 0);

        let mut pair = 0;
        let mut three = 0;
        let mut max_straight_count = 1; // default is 1
        let mut current_straight_count = 1; // default is 1
        let mut straight_min_index: Vec<usize> = vec![];

        // Add first 
        *(type_map.get_mut(&cards[0].card_type).unwrap()) += 1;

        for i in 1..cards.len() {
            // Add suit into hashmap
            *(type_map.get_mut(&cards[i].card_type).unwrap()) += 1;

            // if current number is Increasing
            if cards[i].number - 1 == cards[i-1].number {
                // sustain straightness
                current_straight_count += 1;

                if current_straight_count >= COMB_COUNT {
                    straight_min_index.push(i - (COMB_COUNT - 1));
                }

                // Update max_straight_count
                if max_straight_count < current_straight_count {
                    max_straight_count = current_straight_count;
                }
            } 
            // if current number is the same number as prior element
            else if cards[i].number == cards[i-1].number{
                if i >= 2 && cards[i].number == cards[i-2].number {

                    three += 1;

                    // NOTE!
                    // This is because currently pair is added before threeofkind is detected.
                    // e.g at following order...
                    // [0]. J [1]. J [2]. J
                    // in 1st index it will add pair and also add pair at 2nd index
                    // while adding three by 1 in 2nd index.
                    // this is not desirable thus pair should be decreased by 1
                    // to make three and pair distinctive.
                    pair -= 1;
                } else {
                    pair += 1;
                }
            } 
            // None of the above conditions are met
            else {
                // reset
                current_straight_count = 1;
            }
        }

        // At least straight
        if max_straight_count >= COMB_COUNT {
            // TODO Change this 
            let mut royal_straight_flush = false;
            let mut straight_flush = false;
            let mut straight = false;

            for index in straight_min_index {
                let mut flush_straight = true;
                // Check if royal straight flush or straight flush holds
                // set default suit 
                let suit : CardType = cards[index].card_type;
                for i in index + 1..index + COMB_COUNT {
                    if suit != cards[i].card_type {
                        flush_straight = false;
                        break;
                    }
                }

                if flush_straight {
                    if cards[index].number as usize == CARD_MAX_NUMBER - COMB_COUNT + 1 {
                        // This is royal flush
                        royal_straight_flush = true;
                    } else {
                        // Straight flush
                        straight_flush = true;
                    }
                } 
                // NO flush 
                else {
                    straight = true;
                }
            }

            // Return combinations
            if royal_straight_flush { return (CardCombination::Rflush, None); }
            else if straight_flush { return (CardCombination::Sflush, None); }
            else if straight { return (CardCombination::Straight, None); }

        } 
        // No straight
        else {
            // Check flush
            for (_, value) in type_map.iter() {
                if *value >= COMB_COUNT {
                    return (CardCombination::Flush, None);
                }
            }
        }

        if three >= 1 {
            if pair >= 1 {
                return (CardCombination::FullHouse, None);
            } else {
                return (CardCombination::ThreeOfaKind, None);
            }
        } else {
            if pair >= 2 {
                return (CardCombination::TwoPair, None);
            } else if pair >= 1 {
                return (CardCombination::Pair, None);
            }
        }

        // This is because array is sorted increasing order by number.
        // Thust last card has highest number
        (CardCombination::HighCard, Some(cards[cards.len()-1].number.to_string()))
    }
}
