use std::collections::VecDeque;
use std::net::SocketAddr;
use futures_channel::mpsc::UnboundedSender;
use rand::seq::SliceRandom;
use tokio_tungstenite::tungstenite::Message;
use std::io;

use crate::player::Player;
use crate::room::PlayerInfo;
use crate::player::PlayerState;

mod actionflag;
mod evaluate;

use evaluate::evaluate_hand;
use actionflag::ActionFlag;

pub struct Game {
    players: Vec<Player>,   
    deck: VecDeque<String>, 
    pot: usize,
    board: Vec<String>,
    // Game Info 로 묶어서 관리할 수 있을 듯
    sb_idx: usize, // sb_idx로 변경해도 될 듯
    blind: usize, // big_blind
    // 이 3개를 묶어서 game_player_state 정도로 관리할 수 있을 듯
    have_extra_chips: usize, // raise가 가능한 player 수
    alive: usize, // 살아있는 player 수
    winners: usize, // winner 수
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
                println!("{} has {}, {}", player.name, player.hands.as_ref().unwrap().0, player.hands.as_ref().unwrap().1) 
            }
        }
    }

    pub fn print_player_chips(&self) {
        for player in self.players.iter() {
            println!("{} has {} chips now.", player.name, player.chips)
        }
    }
    
}

impl Game {
    pub fn new(blind: usize) -> Game {
        Game {
            players: Vec::new(),
            deck: Game::init_deck(),
            pot: 0,
            board: Vec::new(),
            sb_idx: 0,
            blind,
            have_extra_chips: 0,
            alive: 0,
            winners: 0,
        }
    }

    pub fn broadcast_game_state() {

    }

    // 새로운 Player를 만들어서 Waiting Queue Push하기
    pub fn insert_player(&mut self, info: PlayerInfo, tx:UnboundedSender<Message>) {
        let player = Player::new(info.name, info.chips, info.addr, tx);
        self.players.push(player);
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

    pub fn game_start(&mut self) {
        self.init_player_state();
        self.reset_game_player_state();
        self.update_game_info();
        self.reset_game();

        // self.print_deck();
        self.print_player_chips();
        

        // run hands
        for player in self.players.iter_mut() {
            
            let card1 = self.deck.pop_front().unwrap();
            let card2 = self.deck.pop_front().unwrap();

            player.hands = Some((card1, card2));
        }

        self.print_player_hands();

        // Free Flops
        self.betting_phase(true);
        if self.is_game_finished() {
            return
        }

        self.set_player_idle();

        // Flop
        for _ in 0..3 {
            self.board.push(self.deck.pop_front().unwrap());
        }

        self.print_board();
        self.print_player_hands();

        self.betting_phase(false);
        if self.is_game_finished() {
            return
        }
        
        self.set_player_idle();

        // Turn
        self.board.push(self.deck.pop_front().unwrap());

        self.print_board();
        self.print_player_hands();
        self.betting_phase(false);

        if self.is_game_finished() {
            return
        }
        
        self.set_player_idle();

        // River
        self.board.push(self.deck.pop_front().unwrap());

        self.print_board();
        self.print_player_hands();
        self.betting_phase(false);

        if self.is_game_finished() {
            return
        }

        self.show_down();
        self.winner_takes_pot();
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

    fn betting_phase(&mut self, is_free_flop: bool) {

        let sb_idx: usize = self.sb_idx;
        let mut cur_idx: usize;
        let mut call_pot: usize; // 일인당 내야하는 총 chip의 개수

        if is_free_flop {

            let bb_idx = (sb_idx + 1) % self.players.len();
            cur_idx = (sb_idx + 2) % self.players.len();

            self.player_blind_raise(sb_idx as usize, self.blind / 2);
            self.player_blind_raise(bb_idx as usize, self.blind);

            call_pot = self.blind; // 왠만하면 string 빼고 다 copy trait 가지고 있다 생각하면 될듯. .clone() 쓸 필요 없음
            
        } else {
            cur_idx = sb_idx;
            call_pot = self.find_largest_player_pot();
        }

        while !self.is_bet_finished(cur_idx as usize, call_pot) { // 함수 보낼 때도 왠만하면 string 빼고 다 copy trait 가지고 있다 생각하면 될듯. & 쓸 필요 없음

            let player = &self.players[cur_idx];
            let mut flag: ActionFlag = ActionFlag::new();

            if !player.is_alive() || player.state == PlayerState::AllIn {
                cur_idx = self.next_idx(cur_idx);
                continue;
            }

            println!("--------------------");
            println!("[Game] {}, Your turn.", player.name);
            println!("[Game] Current Pot-size is {}", self.pot);
            println!("[Game] Your chips amount is {}.", player.chips);
            
            if call_pot == 0 || player.player_pot == call_pot {
                println!("[Game] You can do 1: Check, 3: Raise, 4: AllIn or 5: Fold.");
                flag.call = false;
            } else {
                println!("[Game] You have to bet {} to call.", call_pot - player.player_pot);
                flag.check = false;

                if player.chips + player.player_pot <= call_pot {
                    flag.raise = false;
                    flag.call = false;
                    println!("[Game] You can do 4: AllIn, 5: Fold");
                } else if player.state == PlayerState::Call {
                    flag.raise = false;
                    flag.allin = false;
                    println!("[Game] You can do 2: Call, 5: Fold");
                } else {
                    println!("[Game] You can do 2: Call, 3: Raise, 4: AllIn, 5: Fold")
                }
            }
            
            let mut bet_action: u32;

            loop {
                let mut action = String::new();
                io::stdin().read_line(&mut action).expect("[Game] Read Error");
                bet_action = action.trim().parse().expect("[Game] Please type a number!"); // 이게 u32 이상을 넘어설 수 있으므로 match에서 예외처리를 해줘야한다.

                if flag.can_act(bet_action) {
                    break;
                } else {
                    println!("[Game] Please Type Correct Action..");
                    continue;
                }
            }
            
            call_pot = match bet_action {
                1 => {
                    self.player_check(cur_idx);
                    call_pot
                }
                2 => {
                    self.player_call(cur_idx, call_pot - player.player_pot);
                    call_pot
                }
                3 => {
                    let mut raise_num_buf = String::new();
                    let mut raise_num: usize;

                    loop {
                        println!("[Game] Enter your raise size.");
                        io::stdin().read_line(&mut raise_num_buf).expect("[Game] Read Error");
                        raise_num = raise_num_buf.trim().parse().expect("[Game] Please type a number!");

                        if player.chips < raise_num {
                            println!("[Game] Can't raise with this num! Try again.");
                            continue;
                        } else {
                            break;
                        }
                    }
                    
                    self.player_raise(cur_idx, raise_num)
                },
                4 => {
                    self.player_allin(cur_idx, call_pot)
                }
                5 => {
                    self.player_fold(cur_idx);
                    call_pot
                },
                _ => {
                    println!("Please Enter Correct Number!");
                    call_pot
                }
            };

            cur_idx = (cur_idx + 1) % self.players.len();
        }
    }

    fn is_game_finished(&mut self) -> bool{

        if self.is_early_showdown() {
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

    fn is_bet_finished(&mut self, idx: usize, call_pot: usize) -> bool {

        let player = &self.players[idx];

        if (call_pot == player.player_pot && player.is_acted()) || self.only_one_left() {
            true
        } else {
            false
        }
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
                println!("Winner {} takes {} chips!", player.name, self.pot / self.winners)
            }
            player.player_pot = 0;
        }

        self.pot = 0;
    }

}
