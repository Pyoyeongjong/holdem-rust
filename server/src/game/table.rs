use std::collections::VecDeque;
use rand::prelude::SliceRandom;
use crate::game::error::GameError;

pub struct Table {
    deck: VecDeque<String>,
    board: Vec<String>,
    pub pot: usize,
}

impl Table{
    pub fn new() -> Table {
        Table {
            deck: init_deck(),
            board: Vec::new(),
            pot: 0
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

    pub fn print_board(&self) {
        print!("Board: ");
        for card in self.board.iter() {
            print!("{} ", card);
        }
        println!("");
    }

    pub fn get_board(&self) -> Vec<String> {
        self.board.clone()
    }

    //ok_or: Option<T> 를 Result<T, E> 로 바꿔준다.
    pub fn draw_card(&mut self) -> Result<String, GameError> {
        self.deck.pop_front().ok_or(GameError::NoCardsInDeck)
    }

    pub fn place_card_in_board(&mut self) -> Result<(), GameError> {
        if self.board.len() >= 5 {
            return Err(GameError::BoardFull)
        }
        let card = self.draw_card()?;
        self.board.push(card);
        Ok(())
    }

    pub fn set_board_full(&mut self) -> Result<(), GameError> {
        while self.board.len() < 5 {
            self.place_card_in_board()?;
        }
        Ok(())
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

