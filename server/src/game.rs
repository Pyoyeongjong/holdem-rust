use std::collections::VecDeque;
use std::net::SocketAddr;
use futures_channel::mpsc::UnboundedSender;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

use crate::player::Player;
use crate::room::{GameCommand, PlayerInfo};
use crate::player::PlayerState;
use crate::{db, params};

mod actionflag;
mod evaluate;

use evaluate::evaluate_hand;
use actionflag::ActionFlag;

const MAX_PLAYER: usize = params::MAX_PLAYER;

pub struct Game {
    _id: usize,
    players: Vec<Player>,   
    deck: VecDeque<String>, 
    pot: usize,
    board: Vec<String>,
    // Game Info 로 묶어서 관리할 수 있을 듯
    sb_idx: usize, // sb_idx로 변경해도 될 듯
    blind: usize, // big_blind
    game_state: GameState,
    // 이 3개를 묶어서 game_player_state 정도로 관리할 수 있을 듯
    have_extra_chips: usize, // raise가 가능한 player 수
    alive: usize, // 살아있는 player 수
    winners: usize, // winner 수
    // Game 진행용
    cur_idx: usize,
    call_pot: usize
}

#[derive(Serialize, Deserialize, PartialEq)]
enum GameState {
    Init,
    BeforeStart,
    FreeFlop,
    Flop,
    River,
    Turn,
    ShowDown,
}

#[allow(dead_code)]
impl Game {
    pub fn print_deck(&self) {
        print!("Deck: ");
        for card in self.deck.iter() {
            print!("{} ", card);
        }
        println!("");
    }

    pub fn print_board(&self) {
        print!("Board: ");
        for card in self.board.iter() {
            print!("{} ", card);
        }
        println!("");
    }

    pub fn print_player_hands(&self) {
        for player in self.players.iter() {
            if player.is_alive() { 
                println!("{} has {}, {}", player.id, player.hands.as_ref().unwrap().0, player.hands.as_ref().unwrap().1) 
            }
        }
    }

    pub fn print_player_chips(&self) {
        for player in self.players.iter() {
            println!("{} has {} chips now.", player.id, player.chips)
        }
    }
    
}

impl Game {
    pub fn new(id: usize, blind: usize) -> Game {
        Game {
            _id: id,
            players: Vec::new(),
            deck: Game::init_deck(),
            pot: 0,
            board: Vec::new(),
            sb_idx: 0,
            blind,
            game_state: GameState::Init,
            have_extra_chips: 0,
            alive: 0,
            winners: 0,
            // Game 진행할 때
            cur_idx: 0,
            call_pot: 0,
        }
    }

    pub fn auth_player(&self, id: String) -> bool {
        let player = &self.players[self.cur_idx];
        if id == player.id { true }
        else { false }
    }

    // 얘는 player client로 가는 tx에 쏴준다.
    pub fn broadcast(&self, is_finished: bool) {
        println!("broadcast!");
        
        for player in self.players.iter() {
            let mut state= self.current_game_state(&player.id, is_finished);

            if let Value::Object(obj) = &mut state {
                obj.insert("id".to_string(), Value::String(player.id.clone()));
            }

            println!("Send msg to {}", player.id);
            player.tx.unbounded_send(Message::Text(state.clone().to_string().into())).unwrap();
        }
    }

    fn kick_player(&mut self) {
        let msg = json!({
            "type": "kick"
        });
        for player in self.players.iter() {
            if player.chips <= 0 {
                player.tx.unbounded_send(Message::Text(msg.clone().to_string().into())).unwrap()
            }
        }
    }

    pub fn current_game_state(&self, id: &String, is_finished: bool) -> Value {

        let mut players_json: Vec<Value>= self.players.iter().map(|player| {
            let hands = player.hands.clone()
                .unwrap_or(("??".to_string(), "??".to_string()));
            json!({
                "name": player.id,
                "state": format!("{:?}", player.state),
                "card1": if &player.id == id || is_finished {hands.0} else {"??".to_string()},
                "card2": if &player.id == id || is_finished {hands.1} else {"??".to_string()},
                "chips": player.chips,
                "player_pot": player.player_pot
            })
        }).collect();

        for _ in players_json.len()..MAX_PLAYER {
            players_json.push(Value::Null);
        }

        let cards = self.board.clone();

        let curr_state = json!({
            "type": "game_state",
            "players": players_json,
            "board": {
                "pot": self.pot,
                "bb": self.blind,
                "cards": cards,
                "state": self.game_state,
                "call_pot": self.call_pot,
            }
        });

        curr_state
    }

    pub fn get_players_len(&self) -> usize {
        if self.game_state == GameState::Init {
            return 1;
        }
        self.players.len()
    }

    // 새로운 Player를 만들어서 Waiting Queue Push하기
    pub fn insert_player(&mut self, info: PlayerInfo, tx: UnboundedSender<Message>) {
        if self.game_state == GameState::Init {
            self.game_state = GameState::BeforeStart;
        }
        let player = Player::new(info.name, info.chips, info.addr, tx.clone());
        self.players.push(player);

        println!("Inserted Player! player_len: {}",self.players.len());

    }

    pub fn delete_player_by_addr(&mut self, addr: SocketAddr) {
        for (i, player) in self.players.iter().enumerate() {
            // C에선 안되는데 ㅋㅋ 개꿀
            if player.addr == addr {
                self.players.remove(i);
                println!("Found player and removed! player_len is {}", self.players.len() );
                break;
            }
        }
    }

    fn reset_game_player_state(&mut self) {
        self.have_extra_chips = self.players.len();
        self.alive = self.players.len();
        self.winners = 0;
    }

    fn update_game_info(&mut self) {
        self.sb_idx = (self.sb_idx + 1) % self.players.len();
    }

    pub fn reset_game(&mut self) {
        self.deck = Game::init_deck();
        self.board = Vec::new();
        self.pot = 0;
    }

    pub fn is_game_start(&self) -> bool {
        !(self.game_state == GameState::BeforeStart)
    }

    pub fn is_game_can_start(&self) -> bool {
        self.players.len() >= 2
    }
    pub fn game_start(&mut self) {
        
        self.init_player_state();
        self.reset_game_player_state();
        self.update_game_info();
        self.reset_game();

        self.print_player_chips();
        
        // run hands
        for player in self.players.iter_mut() {
            
            let card1 = self.deck.pop_front().unwrap();
            let card2 = self.deck.pop_front().unwrap();

            player.hands = Some((card1, card2));
        }

        self.print_player_hands();
        self.game_state = GameState::FreeFlop;
        self.betting_phase_init();
    }

    pub fn after_betting(&mut self) {

        if self.is_game_finished() {
            if let Err(e) = db::save_result_in_db(&self.players) {
                eprintln!("DB Save Result in DB Error!: {e}");
            }
            self.kick_player();
            return
        }

        self.set_player_idle();

        // Flop
        match self.game_state {
            GameState::FreeFlop => {
                for _ in 0..3 {
                    self.board.push(self.deck.pop_front().unwrap());
                }
                self.game_state = GameState::Flop;
            },
            GameState::Flop => {
                self.board.push(self.deck.pop_front().unwrap());
                self.game_state = GameState::Turn;
            },
            GameState::Turn => {
                self.board.push(self.deck.pop_front().unwrap());
                self.game_state = GameState::River;
            },
            GameState::River => { self.game_state = GameState::ShowDown; },
            _ => {}
        }

        self.betting_phase_init();
    }

    fn init_player_state(&mut self) {
        for player in self.players.iter_mut() { // iter_mut 으로 가변 참조로 불러옴
            player.change_state(PlayerState::Idle);
            player.hands = None;
        }
    }

    fn init_deck() -> VecDeque<String>{

        let mut deck = Vec::new();
        let mut rng = rand::rng();
        
        let suits = vec!["♠", "◆", "♥", "♣"];
        let ranks = vec!["2", "3", "4", "5", "6", "7", "8", "9", "T", "J", "Q", "K", "A"];
        // for rank in ranks -> 소유권을 가져감
        // for rank in ranks.iter() -> 참조만 함
        for suit in suits.iter() {
            for rank in ranks.iter() {
                deck.push(format!("{}{}", suit, rank)); // format은 참조자를 이용한다. -> 그리고 새로운 String 반환환다.
            }
        };

        deck.shuffle(&mut rng);

        let deck: VecDeque<String> = VecDeque::from(deck);
        deck
    }

    fn betting_phase_init(&mut self) {

        if self.game_state == GameState::FreeFlop {

            let bb_idx = (self.sb_idx + 1) % self.players.len();
            self.cur_idx = (self.sb_idx + 2) % self.players.len();

            self.player_blind_raise(self.sb_idx as usize, self.blind / 2);
            self.player_blind_raise(bb_idx as usize, self.blind);

            self.call_pot = self.blind; // 왠만하면 string 빼고 다 copy trait 가지고 있다 생각하면 될듯. .clone() 쓸 필요 없음
            
        } else {
            // 플랍부터는 sb부터!
            self.cur_idx = self.sb_idx;
            self.call_pot = self.find_largest_player_pot();
        }
    }

    // cur_idx에게만 action annotiation
    pub fn betting_phase_annotation(&mut self) {

        if self.game_state == GameState::BeforeStart {
            return;
        }

        let mut flag: ActionFlag = ActionFlag::new();
        let player = &self.players[self.cur_idx];

        println!("--------------------");
        println!("[Game] {}, Your turn.", player.id);
        println!("[Game] Current Pot-size is {}", self.pot);
        println!("[Game] Your chips amount is {}.", player.chips);

        if self.call_pot == 0 || player.player_pot == self.call_pot {
            println!("[Game] You can do 1: Check, 3: Raise, 4: AllIn or 5: Fold.");
            flag.call = false;
        } else {
            println!("{} {}", self.call_pot, player.player_pot);
            println!("[Game] You have to bet {} to call.", self.call_pot - player.player_pot);
            flag.check = false;

            if player.chips + player.player_pot <= self.call_pot {
                flag.raise = false;
                flag.call = false;
                println!("[Game] You can do 4: AllIn, 5: Fold");
            } else {
                println!("[Game] You can do 2: Call, 3: Raise, 4: AllIn, 5: Fold");
            }
        }

        let state = json!({
            "type": "action",
            "action": flag
        }).to_string();

        player.tx.unbounded_send(Message::Text(state.clone().into())).unwrap();
    }

    pub fn get_next_player_idx(&mut self) -> bool{

        println!("Hi snp");

        loop {

            self.cur_idx = self.next_idx(self.cur_idx);

            if self.is_bet_finished() {
                println!("Set_next_player is bet finished");
                self.after_betting();
                return true;
            }

            let player = &self.players[self.cur_idx];


            if !player.is_alive() || player.state == PlayerState::AllIn {
                self.cur_idx = self.next_idx(self.cur_idx);
                continue;
            } else {
                break;
            }
        }
        false
    }

    pub fn betting_phase_action(&mut self, bet_action: GameCommand) -> bool{

        let player = &self.players[self.cur_idx];

        println!("Hi betting phase action!");

        self.call_pot = match bet_action {
            GameCommand::Check => {
                self.player_check(self.cur_idx);
                self.call_pot
            }
            GameCommand::Call => {
                self.player_call(self.cur_idx, self.call_pot - player.player_pot);
                self.call_pot
            }
            GameCommand::Raise(size) => {

                if player.chips < size {
                    println!("[Game] Can't raise with this num! Try again.");
                    return false;
                } 
                
                self.player_raise(self.cur_idx, size)
            },
            GameCommand::AllIn => {
                self.player_allin(self.cur_idx, self.call_pot)
            },
            GameCommand::Fold => {
                self.player_fold(self.cur_idx);
                self.call_pot
            },
            _ => {
                println!("Please Enter Correct Number!");
                self.call_pot
            }
        };

        self.get_next_player_idx()
    }

    fn is_game_finished(&mut self) -> bool{

        if self.is_early_showdown() || self.game_state == GameState::River {
            self.early_showdown();
            self.print_board();
            self.print_player_hands();
            self.winner_takes_pot();
            return true;
        }

        if self.only_one_left() {
            self.print_board();
            self.print_player_hands();
            for player in self.players.iter_mut() {
                if player.is_alive() {
                    player.change_state(PlayerState::Winner);
                    self.winners += 1;
                }
            }

            self.winner_takes_pot();
            return true;
        }

        false
    }

    fn next_idx(&self, idx: usize) -> usize {
        (idx + 1) % self.players.len()
    }

    /* 둘 이상이 있고, 올인하지 않은 플레이어가 1명 이하일 때 */
    fn is_early_showdown(&self) -> bool {
        if self.alive > 1 && self.have_extra_chips <= 1 {
            return true;
        }
        false
    }

    fn only_one_left(&self) -> bool {
        println!("self.alive = {}", self.alive);
        if self.alive == 1 {
            return true
        }
        false
    }

    fn early_showdown(&mut self) {

        // 보드 다 깔기
        while self.board.len() < 5 {
            self.board.push(self.deck.pop_front().unwrap());
        }

        self.show_down();
    }

    fn set_player_idle(&mut self) {
        for player in self.players.iter_mut() {
            if player.should_return_to_idle() {
                player.state = PlayerState::Idle;
            }
        }
    }

    fn player_raise(&mut self, idx: usize, size: usize) -> usize {

        let is_allin = self.players[idx].raise(size);
        self.pot += size;
        if is_allin { self.have_extra_chips -= 1; }
        self.players[idx].player_pot
    }

    fn player_allin(&mut self, idx: usize, call_pot: usize) -> usize {
        
        self.pot += self.players[idx].chips;
        self.players[idx].allin();
        self.have_extra_chips -= 1;

        if self.players[idx].player_pot > call_pot {
            self.players[idx].player_pot
        } else {
            call_pot
        }
    }

    fn player_blind_raise(&mut self, idx: usize, size: usize) -> usize {
        self.pot += size;
        self.players[idx].blind_raise(size);
        self.players[idx].player_pot
    }

    fn player_call(&mut self, idx: usize, size: usize) {
        self.pot += size;
        self.players[idx].call(size);
    }

    fn player_check(&mut self, idx: usize) {
        self.players[idx].check();
    }

    fn player_fold(&mut self, idx: usize) {
        self.players[idx].fold();
        self.alive -= 1;
        self.have_extra_chips -= 1;
    }

    fn is_bet_finished(&mut self) -> bool {
        let player = &self.players[self.cur_idx];
        if (self.call_pot == player.player_pot && player.is_acted()) || self.only_one_left() { true } 
        else { false }
    }
    
    // Winner를 설정하는 함수
    fn show_down(&mut self) {

        fn compare_hands (player: &Player, winner: &Player, board: &Vec<String>) -> i32 {
            let mut player_cards = board.clone(); // 이건 String이라 clone이 맞다.
            let player_hand = player.hands.clone().unwrap(); // 얘도.
            player_cards.push(player_hand.0);
            player_cards.push(player_hand.1);
    
            let mut winner_cards = board.clone();
            let winner_hand = winner.hands.clone().unwrap();
            winner_cards.push(winner_hand.0);
            winner_cards.push(winner_hand.1);
    
            if evaluate_hand(&player_cards) > evaluate_hand(&winner_cards) {
                1
            } else if evaluate_hand(&player_cards) == evaluate_hand(&winner_cards) {
                0
            } else {
                -1
            }
        }

        // 살아남은 플레이어가 1명일 때
        if self.only_one_left() {
            for player in self.players.iter_mut() {
                if player.is_alive() {
                    player.state = PlayerState::Winner;
                    self.winners = 1;
                    return;
                }
            }
        }

        let mut winners: Vec<&mut Player> = Vec::new();
        let board = self.board.clone();

        for player in self.players.iter_mut() {
            if winners.len() == 0 {
                winners.push(player);
            } else {
                match compare_hands(player, winners[0], &board) {
                    1 => { // 플레이어가 이김
                        winners = Vec::new();
                        winners.push(player);
                    },
                    0 => { winners.push(player);}, // 찹
                    _ => {}
                }
            }
        }

        // 승리자 숫자로 winners_takes_pot 실행할 건데 비효율적인지 검토해봐야 할듯
        self.winners = winners.len();

        for player in winners {
            player.state = PlayerState::Winner;
        }
    }

    fn find_largest_player_pot(&self) -> usize {
        let mut result: usize = 0;
        for player in self.players.iter() {
            if player.is_alive() && player.player_pot > result {
                result = player.player_pot;
            }
        }
        result
    }

    fn find_smallest_player_pot(&self) -> usize {
        let mut result: usize = 0xffffffff;
        for player in self.players.iter() {
            if player.is_alive() && player.player_pot < result {
                result = player.player_pot;
            }
        }
        result
    }

    pub fn winner_takes_pot(&mut self) { // Chop이 날 수 있음 주의!

        let main_pot = self.find_smallest_player_pot();

        for player in self.players.iter_mut() {
            if player.state == PlayerState::Winner && player.player_pot > main_pot {
                let side_pot = player.player_pot - main_pot;
                player.get_chips(side_pot);
                self.pot -= side_pot;
            }
        }
        
        for player in self.players.iter_mut() {
            if player.state == PlayerState::Winner {
                player.chips +=  self.pot / self.winners;
                println!("Winner {} takes {} chips!", player.id, self.pot / self.winners)
            }
            player.player_pot = 0;
        }

        self.pot = 0;
        self.game_state = GameState::BeforeStart;
    }

}
