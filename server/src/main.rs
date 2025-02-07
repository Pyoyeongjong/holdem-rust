use std::{collections::VecDeque, io::Read, ptr::copy_nonoverlapping};
use rand::seq::SliceRandom;
use std::io;

struct Player {
    pub name: String,
    chips: u32,
    state: PlayerState,
    pub hands: Option<(String, String)>,
    player_pot: u32,
}

#[derive(PartialEq)]
enum PlayerState {
    Idle,
    Check,
    Call,
    Raise,
    Fold,
    AllIn,
    Waiting,
}

struct Game {
    players: Vec<Player>,   
    deck: VecDeque<String>, 
    pot: u32,
    board: Vec<String>,
    dealer_idx: usize,
    blind: u32,
}

impl Player {
    fn new(name: String, chips: u32) -> Player {
        Player {
            name,
            chips,
            state: PlayerState::Waiting,
            hands: None,
            player_pot: 0,
        }
    }

    pub fn acted(&self) -> bool {
        match self.state {
            PlayerState::Idle => false,
            PlayerState::Check => true,
            PlayerState::Call => true,
            PlayerState::Raise => true,
            PlayerState::Fold => false,
            PlayerState::AllIn => true,
            PlayerState::Waiting => false,
        }
    }

    pub fn alive(&self) -> bool {
        match self.state {
            PlayerState::Idle => true,
            PlayerState::Check => true,
            PlayerState::Call => true,
            PlayerState::Raise => true,
            PlayerState::Fold => false,
            PlayerState::AllIn => true,
            PlayerState::Waiting => false,
        }
    }

    pub fn check(&mut self) {
        self.state = PlayerState::Check;
    }

    pub fn call(&mut self, size: u32) {
        self.chips -= size;
        self.player_pot += size;
        self.state = PlayerState::Call;
    }

    pub fn raise(&mut self, size: u32) {
        self.chips -= size;
        self.player_pot += size;
        self.state = PlayerState::Raise;
    }

    pub fn blind_raise(&mut self, size: u32) {
        self.chips -= size;
        self.player_pot += size;
    }

    pub fn fold(&mut self) {
        self.state = PlayerState::Fold;
    }

    pub fn allin(&mut self) {
        self.player_pot += self.chips;
        self.chips = 0;
        self.state = PlayerState::AllIn;
    }

}

impl Game {
    fn new(blind: u32) -> Game {
        Game {
            players: Vec::new(),
            deck: Game::init_deck(),
            pot: 0,
            board: Vec::new(),
            dealer_idx: 0,
            blind,
        }
    }

    #[allow(dead_code)]
    pub fn print_deck(&self) {
        print!("Deck: ");
        for card in self.deck.iter() {
            print!("{} ", card);
        }
        println!("");
    }

    #[allow(dead_code)]
    pub fn print_board(&self) {
        print!("Board: ");
        for card in self.board.iter() {
            print!("{} ", card);
        }
        println!("");
    }

    pub fn insert_player(&mut self, name: String, chips: u32) {
        let player = Player {
            name,
            chips,
            state: PlayerState::Waiting,
            hands: None,
            player_pot: 0,
        };

        self.players.push(player);
    }

    pub fn game_start(&mut self) {
        self.init_player_state();
        self.deck = Game::init_deck();
        self.board = Vec::new();
        self.print_deck();
        self.pot = 0;
        self.dealer_idx = (self.dealer_idx + 1) % self.players.len();

        // Free Flop
        for player in self.players.iter_mut() {
            let card1 = self.deck.pop_front().unwrap();
            let card2 = self.deck.pop_front().unwrap();

            player.hands = Some((card1, card2));
        }

        for player in self.players.iter() {
            println!("{} has {}, {}", player.name, player.hands.as_ref().unwrap().0, player.hands.as_ref().unwrap().1);
        }

        self.blind_bet();

        self.betting_phase(true);

        // Flop
        for _ in 0..3 {
            self.board.push(self.deck.pop_front().unwrap());
        }

        self.print_board();
        self.betting_phase(false);

        // Turn
        self.board.push(self.deck.pop_front().unwrap());
        self.print_board();
        self.betting_phase(false);

        // River
        self.board.push(self.deck.pop_front().unwrap());
        self.print_board();
        self.betting_phase(false);

        self.show_down();

    }

    fn init_player_state(&mut self) {
        for player in self.players.iter_mut() { // iter_mut 으로 가변 참조로 불러옴
            player.state = PlayerState::Idle; //
            player.hands = None;
        }
    }

    fn init_deck() -> VecDeque<String>{

        let mut deck = Vec::new();
        let mut rng = rand::rng();
        
        let suits = vec!["♠", "◆", "♥", "♣"];
        let ranks = vec!["2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K", "A"];
        for suit in suits.iter() {
            for rank in ranks.iter() {
                deck.push(format!("{}{}", suit, rank)); // format은 참조자를 이용한다. -> 그리고 새로운 String 반환환다.
            }
        };

        // for rank in ranks -> 소유권을 가져감
        // for rank in ranks.iter() -> 참조만 함

        deck.shuffle(&mut rng);

        let deck: VecDeque<String> = VecDeque::from(deck);
        deck
    }

    fn blind_bet(&mut self) {

    }

    fn betting_phase(&mut self, is_free_flop: bool) {

        let sb_idx = (self.dealer_idx + 1) % self.players.len();
        let mut cur_player_idx: usize;
        let mut call_pot: u32;

        if is_free_flop {

            let bb_idx = (sb_idx + 1) % self.players.len();
            cur_player_idx = (sb_idx + 2) % self.players.len();

            self.player_blind_raise(sb_idx as usize, self.blind / 2);
            self.player_blind_raise(bb_idx as usize, self.blind);

            call_pot = self.blind.clone();
            
            self.pot = ( self.blind as f32 * 1.5 ) as u32;
            
        } else {
            cur_player_idx = sb_idx;
            call_pot = self.find_largest_player_pot();
        }

        while !self.is_bet_finished(cur_player_idx as usize, &call_pot) {

            let player = &self.players[cur_player_idx];

            if !player.alive() {
                println!("{} is Dead", player.name);
                continue;
            }

            println!("{}, choose your action. 1: Check, 2: Call, 3: Raise, 4: Fold.", player.name);
            println!("Your chips amount is {}.", player.chips);
            
            if call_pot == 0 || player.player_pot == call_pot {
                println!("You can check, call, raise, fold.");
            } else {
                println!("You have to bet {} to call... You can call, raise, fold", call_pot - player.player_pot);
                if player.state == PlayerState::Call {
                    println!("You can call or just fold.. ");
                }
            }
            
            let mut action = String::new();
            io::stdin().read_line(&mut action).expect("Read Error");

            let action: u32 = action.trim().parse().expect("Please type a number!"); // 이게 u32 이상을 넘어설 수 있으므로 match에서 예외처리를 해줘야한다.
            
            call_pot = match action {
                1 => self.player_check(cur_player_idx),
                2 => self.player_call(cur_player_idx, call_pot - player.player_pot),
                3 => {
                    let mut raise_num = String::new();
                    let player_chips = player.chips.clone();

                    println!("Enter your raise size..");

                    io::stdin().read_line(&mut raise_num).expect("Read Error");
                    let raise_num = raise_num.trim().parse().expect("Please type a number!");

                    if player_chips < raise_num {
                        panic!("Can't raise with this num!");
                    }
                    
                    self.player_raise(cur_player_idx, raise_num)
                }
                4 => {
                    self.player_fold(cur_player_idx);
                    call_pot
                },
                _ => {
                    println!("Please Enter Correct Number!");
                    call_pot
                }
            };

            cur_player_idx = (cur_player_idx + 1) % self.players.len();
        }

        self.set_player_idle();

    }

    fn set_player_idle(&mut self) {
        for player in self.players.iter_mut() {
            player.state = PlayerState::Idle;
        }
    }

    fn player_raise(&mut self, idx: usize, size: u32) -> u32 {

        self.players[idx].raise(size);
        self.pot += size;

        self.players[idx].player_pot
    }

    fn player_blind_raise(&mut self, idx: usize, size: u32) -> u32 {

        self.players[idx].blind_raise(size);
        self.pot += size;

        self.players[idx].player_pot
    }

    fn player_call(&mut self, idx: usize, size: u32) -> u32 {

        self.players[idx].call(size);
        self.pot += size;

        self.players[idx].player_pot
    }

    fn player_check(&mut self, idx: usize) -> u32 {

        self.players[idx].check();
        self.players[idx].player_pot
    }

    fn player_fold(&mut self, idx: usize) {

        self.players[idx].fold();
    }

    fn is_bet_finished(&mut self, idx: usize, call_pot: &u32) -> bool {

        let player = &self.players[idx];

        if call_pot == &player.player_pot && player.alive() && player.state != PlayerState::Idle {
            true
        } else {
            false
        }
    }

    fn show_down(&mut self) {

    }

    fn find_largest_player_pot(&self) -> u32 {

        let mut result: u32 = 0;

        for player in self.players.iter() {
            if player.player_pot > result {
                result = player.player_pot.clone();
            }
        }

        result
    }

}

fn main() {

    let mut game = Game::new(10);
    game.insert_player("Steve".to_string(), 1000);
    game.insert_player("Peter".to_string(), 1000);
    game.insert_player("ByungHyeok".to_string(), 1000);

    game.game_start();

}

/* 다음 해야할 것들


끝 */