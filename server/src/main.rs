use std::{collections::VecDeque, default};
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
    Winner,
}

struct Game {
    players: Vec<Player>,   
    deck: VecDeque<String>, 
    pot: u32,
    board: Vec<String>,
    dealer_idx: usize,
    blind: u32,
    can_raise: usize,
    alive: usize,
    winners: usize,
}

struct ActionFlag {
    pub check: bool,
    pub call: bool,
    pub fold: bool,
    pub raise: bool,
}

impl ActionFlag {
    fn new(check: bool, call: bool, fold: bool, raise: bool) -> Self {
        Self {
            check,
            call,
            fold,
            raise
        }
    }
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
            PlayerState::Check => true,
            PlayerState::Call => true,
            PlayerState::Raise => true,
            PlayerState::AllIn => true,
            _ => false,
        }
    }

    pub fn should_return_to_idle(&self) -> bool {
        match self.state {
            PlayerState::Check => true,
            PlayerState::Call => true,
            PlayerState::Raise => true,
            _ => false,
        }
    }

    pub fn alive(&self) -> bool {
        match self.state {
            PlayerState::Idle => true,
            PlayerState::Check => true,
            PlayerState::Call => true,
            PlayerState::Raise => true,
            PlayerState::AllIn => true,
            _ => false,
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

    pub fn raise(&mut self, size: u32) -> bool {
        self.chips -= size;
        self.player_pot += size;
        if self.chips == 0 {
            self.state = PlayerState::AllIn;
            true
        } else {
            self.state = PlayerState::Raise;
            false
        }
    }

    pub fn blind_raise(&mut self, size: u32) {
        self.chips -= size;
        self.player_pot += size;
    }

    pub fn fold(&mut self) {
        self.state = PlayerState::Fold;
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
            can_raise: 0,
            alive: 0,
            winners: 0,
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
        self.can_raise = self.players.len();
        self.deck = Game::init_deck();
        self.board = Vec::new();
        self.print_deck();
        self.pot = 0;
        self.dealer_idx = (self.dealer_idx + 1) % self.players.len();
        self.alive = self.players.len();
        self.winners = 0;

        // Free Flop
        for player in self.players.iter_mut() {
            let card1 = self.deck.pop_front().unwrap();
            let card2 = self.deck.pop_front().unwrap();

            player.hands = Some((card1, card2));
        }

        for player in self.players.iter() {
            println!("{} has {}, {}", player.name, player.hands.as_ref().unwrap().0, player.hands.as_ref().unwrap().1);
        }

        self.betting_phase(true);

        if self.is_early_showdown() {
            self.early_showdown();
            self.winner_takes_pot();
            return;
        }

        if self.is_end() {

            for player in self.players.iter_mut() {
                if player.alive() {
                    player.state = PlayerState::Winner;
                    self.winners += 1;
                }
            } 
            self.winner_takes_pot();
            return;
        }

        // Flop
        for _ in 0..3 {
            self.board.push(self.deck.pop_front().unwrap());
        }

        self.print_board();
        self.betting_phase(false);

        if self.is_early_showdown() {
            
            self.early_showdown();
            self.winner_takes_pot();
            return;
        }

        if self.is_end() {

            for player in self.players.iter_mut() {
                if player.alive() {
                    player.state = PlayerState::Winner;
                    self.winners += 1;
                }
            } 
            self.winner_takes_pot();
            return;
        }
        
        self.set_player_idle();

        // Turn
        self.board.push(self.deck.pop_front().unwrap());
        self.print_board();
        self.betting_phase(false);

        if self.is_early_showdown() {
            self.early_showdown();
            self.winner_takes_pot();
            return;
        }

        if self.is_end() {

            for player in self.players.iter_mut() {
                if player.alive() {
                    player.state = PlayerState::Winner;
                    self.winners += 1;
                }
            } 
            self.winner_takes_pot();
            return;
        }

        // River
        self.board.push(self.deck.pop_front().unwrap());
        self.print_board();
        self.betting_phase(false);

        if self.is_early_showdown() {
            self.early_showdown();
            self.winner_takes_pot();
            return;
        }

        if self.is_end() {

            for player in self.players.iter_mut() {
                if player.alive() {
                    player.state = PlayerState::Winner;
                    self.winners += 1;
                }
            } 
            self.winner_takes_pot();
            return;
        }

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
        let ranks = vec!["2", "3", "4", "5", "6", "7", "8", "9", "T", "J", "Q", "K", "A"];
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

            println!("--------------------");

            let player = &self.players[cur_player_idx];

            let mut flag: ActionFlag = ActionFlag::new(true, true, true, true);

            if !player.alive() {
                println!("{} is Dead", player.name);
                cur_player_idx = (cur_player_idx + 1) % self.players.len();
                continue;
            }

            if player.state == PlayerState::AllIn {
                println!("{} is All-in state", player.name);
                cur_player_idx = (cur_player_idx + 1) % self.players.len();
                continue;
            }

            println!("Current Pot-size is {}", self.pot);

            println!("{}, choose your action. 1: Check, 2: Call, 3: Raise, 4: Fold.", player.name);
            println!("Your chips amount is {}.", player.chips);
            
            if call_pot == 0 || player.player_pot == call_pot {
                println!("You can check, raise, or fold.");
                flag.call = false;
            } else {
                println!("You have to bet {} to call... You can call, raise, fold", call_pot - player.player_pot);
                flag.check = false;
                if player.state == PlayerState::Call {
                    flag.call = false;
                    println!("You can call or just fold.. ");
                }
            }
            
            let mut bet_action: u32;

            loop {
                
                let mut action = String::new();
                io::stdin().read_line(&mut action).expect("Read Error");

                bet_action = action.trim().parse().expect("Please type a number!"); // 이게 u32 이상을 넘어설 수 있으므로 match에서 예외처리를 해줘야한다.

                match bet_action {
                    1 => if flag.check == true {
                        break;
                    }
                    2 => if flag.call == true {
                        break;
                    }
                    3 => if flag.raise == true {
                        break;
                    }
                    4 => if flag.fold == true {
                        break;
                    }

                    _ => {
                        println!("Please Type Correct Action..");
                        continue;
                    }
                }

                println!("You can't Act this.. Try again");
            }
            
            call_pot = match bet_action {
                1 => self.player_check(cur_player_idx),
                2 => self.player_call(cur_player_idx, call_pot - player.player_pot),
                3 => {
                    let mut raise_num_buf = String::new();
                    let mut raise_num: u32;
                    let player_chips = player.chips.clone();

                    loop {
                        println!("Enter your raise size..");

                        io::stdin().read_line(&mut raise_num_buf).expect("Read Error");
                        raise_num = raise_num_buf.trim().parse().expect("Please type a number!");

                        if player_chips < raise_num {
                            println!("Can't raise with this num!");
                            continue;
                        } else {
                            break;
                        }
                    }
                    
                    self.player_raise(cur_player_idx, raise_num)
                }
                4 => {
                    self.player_fold(cur_player_idx);
                    self.alive -= 1;
                    self.can_raise -= 1;
                    call_pot
                },
                _ => {
                    println!("Please Enter Correct Number!");
                    call_pot
                }
            };

            cur_player_idx = (cur_player_idx + 1) % self.players.len();
        }

    }

    pub fn winner_takes_pot(&mut self) { // Chop이 날 수 있음 주의!
        for player in self.players.iter_mut() {
            println!("Player {} ...", player.name);
            if player.state == PlayerState::Winner {
                player.chips +=  self.pot / self.winners as u32;
                println!("Winner {} takes {} chips!", player.name, self.pot / self.winners as u32)
            }
            player.player_pot = 0;
        }

    }

    fn is_early_showdown(&self) -> bool {
        if self.alive > 1 && self.can_raise <= 1 {
            println!("early showdown!");
            return true;
        }
        false
    }

    fn is_end(&self) -> bool {
        if self.alive == 1 {
            println!("is end~~");
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

    fn player_raise(&mut self, idx: usize, size: u32) -> u32 {

        let is_allin = self.players[idx].raise(size);
        if is_allin {
            self.can_raise -= 1;
        }
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

        if call_pot == &player.player_pot && player.alive() && player.state != PlayerState::Idle || self.is_end() {
            true
        } else {
            false
        }
    }

    fn show_down(&mut self) {

        fn compare_hands (player: &Player, winner: &Player, board: &Vec<String>) -> i32 {
            let mut player_cards = board.clone();
            let player_hand = player.hands.clone().unwrap();
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

        if self.alive == 1 {
            for player in self.players.iter_mut() {
                if player.alive() {
                    player.state = PlayerState::Winner;
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
                    1 =>{
                        winners = Vec::new();
                        winners.push(player);
                    },
                    0 =>{
                        winners.push(player);
                    },
                    _ => {

                    }
                }
            }
        }

        self.winners = winners.len();
        for player in winners {
            player.state = PlayerState::Winner;
        }


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

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
enum HandRank {
    TopCard(u8, u8, u8, u8, u8),
    Pair(u8, u8, u8, u8),
    TwoPairs(u8, u8, u8),
    ThreeofCards(u8, u8, u8),
    Straight(u8),
    Flush(u8, u8, u8, u8, u8),
    FullHouse(u8, u8),
    FourOfCards(u8, u8),
    StraigntFlush(u8, u8, u8, u8, u8),
}

fn is_straight(vec: &Vec<u8>) -> Option<[u8; 5]> {
    if vec.len() < 5 {
        None
    } else {

        let mut idx = 0;
        let mut prev = vec[idx];
        idx += 1;
        let mut count = 1;

        while idx < vec.len() && count < 5 {
            
            let cur = vec[idx];

            if prev - 1 == cur {
                count += 1;
            }
            else {
                count = 1;
            }
            prev = cur;
            idx += 1;
        }

        if count >= 5 {
            Some([vec[idx-5], vec[idx-4], vec[idx-3], vec[idx-2], vec[idx-1]])
        } else {
            None
        }
    }
}

fn eval_most(vec: &mut [u8; 15], cond: u8) -> Option<u8> {
    let mut max: u8 = 0;
    for i in 2..vec.len() {
        if vec[i] >= cond {
            max = i as u8;
        }
    }

    if max == 0 {
        None
    } else {
        vec[max as usize] = 0;
        Some(max)
    }  
}

fn evaluate_hand(vec: &Vec<String>) -> HandRank {

    let cards = vec;

    let mut suits: [Vec<u8>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    let mut ranks: [u8; 15] = [0; 15];
    let mut card_orders: Vec<u8> = Vec::new();
    
    for card in cards {
        let suit = card.chars().nth(0).unwrap();
        let num = card.chars().nth(1).unwrap();//.to_digit(10).unwrap() as u8;

        let num: u8 = match num {
            'T' => 10,
            'J' => 11,
            'Q' => 12,
            'K' => 13,
            'A' => 14,
            other => other.to_digit(10).unwrap() as u8,
        };

        let mut has_same: bool = false;
        for i in 0..card_orders.len() {
            if card_orders[i] as u8 == num {
                has_same = true;
            }
        }
        if !has_same {
            card_orders.push(num); // copy trait을 가진 타입은 복사되고(u8, i32 등), 그렇지 않으면 소유권 이동(&str);
        }
        has_same = false;

        ranks[num as usize] += 1;

        match suit {
            '♠' => suits[0].push(num),
            '◆' => suits[1].push(num),
            '♥' => suits[2].push(num),
            '♣' => suits[3].push(num),
            _ => panic!("What is this card~~!!"),
        }
    }

    for suit in suits.iter_mut() {
        suit.sort_by(|a, b| b.cmp(a));
    }

    card_orders.sort_by(|a, b| b.cmp(a));

    // print Debug
    for suit in suits.iter() {
        print!("[");
        for num in suit.iter() {
            print!("{num},");
        }
        println!("]");
    }

    print!("[");
    for rank in ranks.iter() {
        print!("{rank}, ");
    }
    println!("]");

    print!("[");
    for card in card_orders.iter() {
        print!("{card} ");
    }
    println!("]");

    // 스티플
    //println!("@@Stifle@@");
    for suit in suits.iter() {
        let straight = is_straight(suit);
        if straight.is_some() {
            let straight = straight.unwrap();
            return HandRank::StraigntFlush(straight[0], straight[1], straight[2], straight[3], straight[4]);
        }
    }
    
    
    // 포카드
    //println!("@@FourCard@@");
    let mut ranks_clone = ranks.clone();
    let first: Option<u8>;
    let second: Option<u8>;
    first = eval_most(&mut ranks_clone, 4);
    if first.is_some() {
        second = eval_most(&mut ranks_clone, 1);
        if second.is_some() {
            return HandRank::FourOfCards(first.unwrap(), second.unwrap());
        }
    }

    // 풀하우스
    //println!("@@FullHouse@@");
    let mut ranks_clone = ranks.clone();
    let first: Option<u8>;
    let second: Option<u8>;
    first = eval_most(&mut ranks_clone, 3);
    if first.is_some() {
        second = eval_most(&mut ranks_clone, 2);
        if second.is_some() {
            return HandRank::FullHouse(first.unwrap(), second.unwrap());
        }
    }
    

    // 플러시
    //println!("@@Flush@@");
    for suit in suits {
        if suit.len() >= 5 {
            return HandRank::Flush(suit[0], suit[1], suit[2], suit[3], suit[4]);
        }
    }

    // 스트레이트
    //println!("@@Straight@@");
    let straight = is_straight(&card_orders);
    if straight.is_some() {
        return HandRank::Straight(straight.unwrap()[0]);
    }

    // 트리플
    //println!("@@Triple@@");
    let mut ranks_clone = ranks.clone();
    let first: Option<u8>;
    let second: Option<u8>;
    let third: Option<u8>;
    first = eval_most(&mut ranks_clone, 3);
    if first.is_some() {
        second = eval_most(&mut ranks_clone, 1);
        if second.is_some() {
            third = eval_most(&mut ranks_clone, 1);
            if third.is_some() {
                return HandRank::ThreeofCards(first.unwrap(), second.unwrap(), third.unwrap());
            }
        }
    }

    // 투페어
    //println!("@@Twopair@@");
    let mut ranks_clone = ranks.clone();
    let first: Option<u8>;
    let second: Option<u8>;
    let third: Option<u8>;
    first = eval_most(&mut ranks_clone, 2);
    if first.is_some() {
        second = eval_most(&mut ranks_clone, 2);
        if second.is_some() {
            third = eval_most(&mut ranks_clone, 1);
            if third.is_some() {
                return HandRank::TwoPairs(first.unwrap(), second.unwrap(), third.unwrap());
            }
        }
    }

    // 페어
    //println!("@@Pair@@");
    let mut ranks_clone = ranks.clone();
    let first: Option<u8>;
    let second: Option<u8>;
    let third: Option<u8>;
    let fourth: Option<u8>;
    first = eval_most(&mut ranks_clone, 2);
    if first.is_some() {
        second = eval_most(&mut ranks_clone, 1);
        if second.is_some() {
            third = eval_most(&mut ranks_clone, 1);
            if third.is_some() {
                fourth = eval_most(&mut ranks_clone, 1);
                if fourth.is_some() {
                    return HandRank::Pair(first.unwrap(), second.unwrap(), third.unwrap(), fourth.unwrap());
                }
            }
        }
    }

    // 탑
    //println!("@@Top@@");
    let mut ranks_clone = ranks.clone();
    let first: Option<u8>;
    let second: Option<u8>;
    let third: Option<u8>;
    let fourth: Option<u8>;
    let last: Option<u8>;
    first = eval_most(&mut ranks_clone, 1);
    if first.is_some() {
        second = eval_most(&mut ranks_clone, 1);
        if second.is_some() {
            third = eval_most(&mut ranks_clone, 1);
            if third.is_some() {
                fourth = eval_most(&mut ranks_clone, 1);
                if fourth.is_some() {
                    last = eval_most(&mut ranks_clone, 1);
                    if last.is_some() {
                        return HandRank::TopCard(first.unwrap(), second.unwrap(), third.unwrap(), fourth.unwrap(), last.unwrap());
                    }
                }
            }
        }
    }

    HandRank::TopCard(0, 0, 0, 0, 0)

}

fn make_cards() -> Vec<String>{
    let mut deck = Vec::new();
    let mut rng = rand::rng();
    
    let suits = vec!["♠", "◆", "♥", "♣"];
    let ranks = vec!["2", "3", "4", "5", "6", "7", "8", "9", "T", "J", "Q", "K", "A"];
    for suit in suits.iter() {
        for rank in ranks.iter() {
            deck.push(format!("{}{}", suit, rank)); // format은 참조자를 이용한다. -> 그리고 새로운 String 반환환다.
        }
    };

    // for rank in ranks -> 소유권을 가져감
    // for rank in ranks.iter() -> 참조만 함

    deck.shuffle(&mut rng);

    let mut deck: VecDeque<String> = VecDeque::from(deck);
    let mut cards: Vec<String> = Vec::new();
    for _ in 0..7 {
        cards.push(deck.pop_front().unwrap());
    }
    cards
}


fn main() {

    let mut game = Game::new(10);
    game.insert_player("Steve".to_string(), 1000);
    game.insert_player("Peter".to_string(), 1000);
    game.insert_player("ByungHyeok".to_string(), 1000);

    game.game_start();
    
    // let cards = make_cards();
    // println!("{:?}", cards);

    // println!("{:?}", evaluate_hand(cards));

}

/* 다음 해야할 것들

1. winner_takes_pot 구현
2. showdown 구현
3. 코드 리팩토링

끝 */

