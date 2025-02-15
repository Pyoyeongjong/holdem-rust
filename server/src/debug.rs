/* Debug functions! */

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