use std::collections::VecDeque;
use rand::seq::SliceRandom;

struct Player {
    pub name: String,
    chips: u32,
    state: PlayerState,
    pub hands: Option<(String, String)>,
}

enum PlayerState {
    Dead,
    Alive,
    Waiting,
}

struct Game {
    players: Vec<Player>,
    deck: VecDeque<String>,
    pot: u32,
    board: Vec<String>,
    dealer_idx: u32,
}

impl Player {
    fn new(name: String, chips: u32) -> Player {
        Player {
            name,
            chips,
            state: PlayerState::Waiting,
            hands: None,
        }
    }
}

impl Game {
    fn new() -> Game {
        Game {
            players: Vec::new(),
            deck: Game::init_deck(),
            pot: 0,
            board: Vec::new(),
            dealer_idx: 0,
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
        };

        self.players.push(player);
    }

    pub fn game_start(&mut self) {
        self.init_player_state();
        self.deck = Game::init_deck();
        self.board = Vec::new();
        self.print_deck();

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

        self.betting_phase();

        // Flop
        for _ in 0..3 {
            self.board.push(self.deck.pop_front().unwrap());
        }

        self.print_board();
        self.betting_phase();

        // Turn
        self.board.push(self.deck.pop_front().unwrap());
        self.print_board();
        self.betting_phase();

        // River
        self.board.push(self.deck.pop_front().unwrap());
        self.print_board();
        self.betting_phase();

        self.show_down();

    }

    fn init_player_state(&mut self) {
        for player in self.players.iter_mut() { // iter_mut 으로 가변 참조로 불러옴
            player.state = PlayerState::Alive; //
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

    fn betting_phase(&mut self) {
        
    }

    fn show_down(&mut self) {

    }

}

fn main() {

    let mut game = Game::new();
    game.insert_player("Steve".to_string(), 1000);
    game.insert_player("Peter".to_string(), 1000);
    game.insert_player("ByungHyeok".to_string(), 1000);

    game.game_start();
    game.game_start();

}

