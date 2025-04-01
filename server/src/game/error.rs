#[derive(Debug)]
pub enum GameError{
    NoCardsInDeck,
    BoardFull,
    PlayerFull,
    PlayerNotFound
}