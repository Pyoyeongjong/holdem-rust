
// main.rs 가 최상위 모듈이라 다른 rs를 확인 가능하지만, game.rs에서는 player.rs를 인식할 수 없음.
// lib.rs로 묶어주거나, game 폴더 안에 player.rs를 구현하거나...
mod game;
mod player;

use game::Game;

fn main() {

    let mut game = Game::new(10);
    game.insert_player("Steve".to_string(), 1000);
    game.insert_player("Peter".to_string(), 1000);
    game.insert_player("ByungHyeok".to_string(), 1000);

    loop {
        game.game_start();
    }
}

/* 다음 해야할 것들

    현재 코드 테스팅하기
    플레이어 추가/제거 Waiting Queue 구현하기

*/ 


