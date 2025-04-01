use std::sync::Arc;

use tokio::sync::RwLock;

use crate::room::room::Room;

use super::types::RoomInfo;

pub struct RoomManager{
    rooms: Vec<Room>,
    max_size: usize,
}

impl RoomManager {
    pub fn new(size: usize) -> RoomManager {

        let rooms = Vec::with_capacity(size);
        // Game Thread가 종료하면서 room_thread에게 room 청소해달라고 요청할 거임

        RoomManager { rooms, max_size: size }
    }

    pub fn craete_new_room(&mut self, name: &str, blind: usize, room_thread_pool: Arc<RwLock<RoomManager>>) -> bool {

        if self.rooms.len() >= self.max_size || name == "" || blind <= 0 || blind % 10 > 0 {

            println!("Create New Room Failed. {} {}", name, blind % 10);
            return false
        }

        let new_room = Room::new(name.to_string(), blind, room_thread_pool);
        self.rooms.push(new_room);

        println!("Create New Room Succeed. Size of Rooms is {}", self.rooms.len());
        true
    }

    // 일단은 vector 순회로 하시죠
    pub fn find_room_by_id(&mut self, id: usize) -> Option<&mut Room> {
        self.find_room(id)
    }

    pub fn get_rooms_info(&self) -> Vec<RoomInfo>{

        let mut rooms_info = Vec::new();
        for room in self.rooms.iter() {
            rooms_info.push(room.room_info.clone());
        }

        rooms_info
    }

    fn find_room(&mut self, id: usize) -> Option<&mut Room> {
        // 설탕 달달하네..
        self.rooms.iter_mut().find(|room| room.id == id)

    }

    pub fn delete_room(&mut self, idx: usize) {

        println!("Hello Delete Room idx={idx}");

        for (room_idx, room) in self.rooms.iter_mut().enumerate() {
            if room.id == idx {
                self.rooms.remove(room_idx);
                return;
            }
        }
    }
}
