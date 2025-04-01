use std::net::SocketAddr;
use futures_channel::mpsc::UnboundedSender;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

use crate::game::player::Player;
use crate::room::types::{GameCommand,PlayerInfo};
use crate::game::player::PlayerState;
use crate::utils::{db, config};

use crate::game::{types, rank};

use rank::rank_hand;
use types::PlayerAction;

use crate::game::table::Table;

use super::error::GameError;
use super::player_manager::PlayerManager;
use super::types::GameState;

const MAX_PLAYER: usize = config::MAX_PLAYER;

pub struct Game {
    _id: usize,
    players: PlayerManager,
    table: Table,
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

impl Game {
    pub fn new(id: usize, blind: usize) -> Game {
        Game {
            _id: id,
            players: PlayerManager::new(MAX_PLAYER),
            table: Table::new(),
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
        let players = self.players.get_players();
        let player = &players[self.cur_idx];
        if id == player.id { true }
        else { false }
    }

    // game에 따라 가려야 하는 정보가 있어서 game 자체에서 관장해야 할듯
    pub fn broadcast(&self, is_finished: bool) {
        println!("broadcast!");
        let players = self.players.get_players();
        
        for player in players.iter() {
            let mut state= self.current_game_state(&player.id, is_finished);

            if let Value::Object(obj) = &mut state {
                obj.insert("id".to_string(), Value::String(player.id.clone()));
            }

            println!("Send msg to {}", player.id);
            player.tx.unbounded_send(Message::Text(state.clone().to_string().into())).unwrap();
        }
    }

    pub fn current_game_state(&self, id: &String, is_finished: bool) -> Value {

        let players = self.players.get_players();
        let mut players_json: Vec<Value>= players.iter().map(|player| {
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

        let cards = self.table.get_board();

        let curr_state = json!({
            "type": "game_state",
            "players": players_json,
            "board": {
                "pot": self.table.pot,
                "bb": self.blind,
                "cards": cards,
                "state": self.game_state,
                "call_pot": self.call_pot,
            }
        });

        curr_state
    }

    pub fn players_len(&self) -> usize {
        if self.game_state == GameState::Init {
            return 1;
        }
        self.players.get_players_len()
    }

    // 새로운 Player를 만들어서 Waiting Queue Push하기
    pub fn insert_player(&mut self, info: PlayerInfo, tx: UnboundedSender<Message>) -> Result<(), GameError>{
        if self.game_state == GameState::Init { self.game_state = GameState::BeforeStart; }
        let player = Player::new(info.name, info.chips, info.addr, tx.clone());
        self.players.add_player(player)
    }

    pub fn delete_player(&mut self, addr: SocketAddr) -> Result<(), GameError>{
        self.players.remove_player_by_addr(addr)
    }

    fn reset_game_player_state(&mut self) {
        self.have_extra_chips = self.players.get_players_len();
        self.alive = self.players.get_players_len();
        self.winners = 0;
    }

    fn update_game_info(&mut self) {
        self.sb_idx = (self.sb_idx + 1) % self.players.get_players_len();
    }

    pub fn reset_game(&mut self) {
        self.table = Table::new();
    }

    pub fn is_game_start(&self) -> bool {
        !(self.game_state == GameState::BeforeStart)
    }

    pub fn is_game_can_start(&self) -> bool {
        self.players.get_players_len() >= 2
    }
    pub fn game_start(&mut self) -> Result<(), GameError>{
        
        self.players.init_player_state();
        self.reset_game_player_state();
        self.update_game_info();
        self.reset_game();

        self.players.print_player_chips();
        
        // run hands
        for player in self.players.get_players_mut().iter_mut() {
            let card1 = self.table.draw_card()?;
            let card2 = self.table.draw_card()?;
            player.hands = Some((card1, card2));
        }

        self.players.print_player_hands();
        self.game_state = GameState::FreeFlop;
        self.betting_phase_init();

        Ok(())
    }

    pub fn after_betting(&mut self) -> Result<bool, GameError> {


        if self.is_game_finished()? {
            println!("game finished!");
            if let Err(e) = db::save_result_in_db(&self.players.get_players()) {
                eprintln!("DB Save Result in DB Error!: {e}");
            }
            self.players.kick_player();
            return Ok(true)
        }

        self.players.set_player_idle();

        // Flop
        match self.game_state {
            GameState::FreeFlop => {
                for _ in 0..3 {
                    self.table.place_card_in_board()?;
                }
                self.game_state = GameState::Flop;
            },
            GameState::Flop => {
                self.table.place_card_in_board()?;
                self.game_state = GameState::Turn;
            },
            GameState::Turn => {
                self.table.place_card_in_board()?;
                self.game_state = GameState::River;
            },
            GameState::River => { self.game_state = GameState::ShowDown; },
            _ => {}
        }

        self.betting_phase_init();
        Ok(false)
    }

    fn betting_phase_init(&mut self) {

        if self.game_state == GameState::FreeFlop {

            let bb_idx = (self.sb_idx + 1) % self.players.get_players_len();
            self.cur_idx = (self.sb_idx + 2) % self.players.get_players_len();

            self.handle_player_blind_raise(self.sb_idx as usize, self.blind / 2);
            self.handle_player_blind_raise(bb_idx as usize, self.blind);

            self.call_pot = self.blind; // 왠만하면 string 빼고 다 copy trait 가지고 있다 생각하면 될듯. .clone() 쓸 필요 없음
            
        } else {
            // 플랍부터는 sb부터!
            self.cur_idx = self.sb_idx;
            self.call_pot = self.players.find_largest_player_pot();
        }
    }

    // cur_idx에게만 action annotiation
    pub fn betting_phase_annotation(&mut self) {

        if self.game_state == GameState::BeforeStart {
            return;
        }

        let mut flag: PlayerAction = PlayerAction::new();
        let player = self.players.get_player_by_idx(self.cur_idx);

        println!("--------------------");
        println!("[Game] {}, Your turn.", player.id);
        println!("[Game] Current Pot-size is {}", self.table.pot);
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

    pub fn get_next_player_idx(&mut self) -> Result<bool, GameError>{

        loop {
            self.cur_idx = self.next_idx(self.cur_idx);

            if self.is_bet_finished() {
                println!("Set_next_player is bet finished");
                return self.after_betting()
            }

            let player = self.players.get_player_by_idx(self.cur_idx);

            if !player.is_alive() || player.state == PlayerState::AllIn {
                self.cur_idx = self.next_idx(self.cur_idx);
                continue;
            } else {
                break;
            }
        }
        Ok(false)
    }

    pub fn betting_phase_action(&mut self, bet_action: GameCommand) -> Result<bool, GameError> {

        let player = self.players.get_player_by_idx(self.cur_idx);

        println!("Hi betting phase action!");

        self.call_pot = match bet_action {
            GameCommand::Check => {
                self.handle_player_check(self.cur_idx);
                self.call_pot
            }
            GameCommand::Call => {
                self.handle_player_call(self.cur_idx, self.call_pot - player.player_pot);
                self.call_pot
            }
            GameCommand::Raise(size) => {

                if player.chips < size {
                    println!("[Game] Can't raise with this num! Try again.");
                    return Ok(false);
                } 
                
                self.handle_player_raise(self.cur_idx, size)
            },
            GameCommand::AllIn => {
                self.handle_player_allin(self.cur_idx, self.call_pot)
            },
            GameCommand::Fold => {
                self.handle_player_fold(self.cur_idx);
                self.call_pot
            },
            _ => {
                println!("Please Enter Correct Number!");
                self.call_pot
            }
        };

        self.get_next_player_idx()
        
    }

    fn is_game_finished(&mut self) -> Result<bool, GameError>{

        if self.is_early_showdown() || self.game_state == GameState::River {
            self.early_showdown()?;
            self.table.print_board();
            self.players.print_player_hands();
            self.winner_takes_pot();
            return Ok(true);
        }

        if self.only_one_left() {
            self.table.print_board();
            self.players.print_player_hands();
            let players = self.players.get_players_mut();
            for player in players.iter_mut() {
                if player.is_alive() {
                    player.change_state(PlayerState::Winner);
                    self.winners += 1;
                }
            }

            self.winner_takes_pot();
            return Ok(true);
        }

        Ok(false)
    }

    fn next_idx(&self, idx: usize) -> usize {
        (idx + 1) % self.players.get_players_len()
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

    fn early_showdown(&mut self) -> Result<(), GameError>{
        self.table.set_board_full()?;
        self.show_down();
        Ok(())
    }

    fn handle_player_raise(&mut self, idx: usize, size: usize) -> usize {

        let player = self.players.get_player_by_idx_mut(idx);

        let is_allin = player.raise(size);
        self.table.pot += size;
        if is_allin { self.have_extra_chips -= 1; }

        player.player_pot
    }

    fn handle_player_allin(&mut self, idx: usize, call_pot: usize) -> usize {

        let player = self.players.get_player_by_idx_mut(idx);
        
        self.table.pot += player.chips;
        player.allin();
        self.have_extra_chips -= 1;

        if player.player_pot > call_pot {
            player.player_pot
        } else {
            call_pot
        }
    }

    fn handle_player_blind_raise(&mut self, idx: usize, size: usize) -> usize {

        let player = self.players.get_player_by_idx_mut(idx);

        self.table.pot += size;
        player.blind_raise(size);
        player.player_pot
    }

    fn handle_player_call(&mut self, idx: usize, size: usize) {

        let player = self.players.get_player_by_idx_mut(idx);

        self.table.pot += size;
        player.call(size);
    }

    fn handle_player_check(&mut self, idx: usize) {

        let player = self.players.get_player_by_idx_mut(idx);

        player.check();
    }

    fn handle_player_fold(&mut self, idx: usize) {

        let player = self.players.get_player_by_idx_mut(idx);

        player.fold();
        self.alive -= 1;
        self.have_extra_chips -= 1;
    }

    fn is_bet_finished(&mut self) -> bool {
        let player = self.players.get_player_by_idx(self.cur_idx);
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
    
            if rank_hand(&player_cards) > rank_hand(&winner_cards) {
                1
            } else if rank_hand(&player_cards) == rank_hand(&winner_cards) {
                0
            } else {
                -1
            }
        }

        // 살아남은 플레이어가 1명일 때
        if self.only_one_left() {
            let players = self.players.get_players_mut();
            for player in players.iter_mut() {
                if player.is_alive() {
                    player.state = PlayerState::Winner;
                    self.winners = 1;
                    return;
                }
            }
        }

        let mut winners: Vec<&mut Player> = Vec::new();
        let board = self.table.get_board();
        let players = self.players.get_players_mut();

        for player in players.iter_mut() {
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

    pub fn winner_takes_pot(&mut self) { // Chop이 날 수 있음 주의!

        let main_pot = self.players.find_smallest_player_pot();
        let players = self.players.get_players_mut();

        for player in players.iter_mut() {
            if player.state == PlayerState::Winner && player.player_pot > main_pot {
                let side_pot = player.player_pot - main_pot;
                player.get_chips(side_pot);
                self.table.pot -= side_pot;
            }
        }
        
        for player in players.iter_mut() {
            if player.state == PlayerState::Winner {
                player.chips +=  self.table.pot / self.winners;
                println!("Winner {} takes {} chips!", player.id, self.table.pot / self.winners)
            }
            player.player_pot = 0;
        }

        self.table.pot = 0;
        self.game_state = GameState::BeforeStart;
    }

}
