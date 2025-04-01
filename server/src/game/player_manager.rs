use std::net::SocketAddr;

use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

use crate::game::player::Player;

use super::{error::GameError, player::PlayerState};

pub struct PlayerManager {
    players: Vec<Player>,
    max_player: usize
}

impl PlayerManager {
    pub fn new(max_player: usize) -> PlayerManager{
        PlayerManager { 
            players: Vec::new(),
            max_player
        }
    }

    pub fn get_player_by_idx(&self, cur_idx: usize) -> &Player{ &self.players[cur_idx] }
    pub fn get_player_by_idx_mut(&mut self, cur_idx: usize) -> &mut Player{ &mut self.players[cur_idx] }
    pub fn get_players(&self) -> &Vec<Player>{ &self.players }
    pub fn get_players_mut(&mut self) -> &mut Vec<Player>{ &mut self.players }
    pub fn get_players_len(&self) -> usize{ self.players.len() }

    pub fn kick_player(&mut self) {
        let msg = json!({
            "type": "kick"
        });
        for player in self.players.iter() {
            if player.chips <= 0 {
                player.tx.unbounded_send(Message::Text(msg.clone().to_string().into())).unwrap()
            }
        }
    }

    pub fn add_player(&mut self, player: Player) -> Result<(), GameError>{
        if self.players.len() >= self.max_player {
            return Err(GameError::PlayerFull);
        } else {
            self.players.push(player);
            Ok(())
        }
    }

    pub fn remove_player_by_addr(&mut self, addr: SocketAddr) -> Result<(), GameError>{
        for (i, player) in self.players.iter().enumerate() {
            // C에선 안되는데 ㅋㅋ 개꿀
            if player.addr == addr {
                self.players.remove(i);
                return Ok(())
            }
        }
        Err(GameError::PlayerNotFound)
    }

    pub fn init_player_state(&mut self) {
        for player in self.players.iter_mut() { // iter_mut 으로 가변 참조로 불러옴
            player.change_state(PlayerState::Idle);
            player.hands = None;
        }
    }

    pub fn set_player_idle(&mut self) {
        for player in self.players.iter_mut() {
            if player.should_return_to_idle() {
                player.state = PlayerState::Idle;
            }
        }
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

    pub fn find_largest_player_pot(&self) -> usize {
        let mut result: usize = 0;
        for player in self.players.iter() {
            if player.is_alive() && player.player_pot > result {
                result = player.player_pot;
            }
        }
        result
    }

    pub fn find_smallest_player_pot(&self) -> usize {
        let mut result: usize = 0xffffffff;
        for player in self.players.iter() {
            if player.is_alive() && player.player_pot < result {
                result = player.player_pot;
            }
        }
        result
    }
}