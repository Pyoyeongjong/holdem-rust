use serde_json::Value;

use crate::room::{Room, RoomInfo};
pub struct RoomThreadPool{
    rooms: Vec<Room>,
    max_size: usize,
}

impl RoomThreadPool {
    pub fn new(size: usize) -> RoomThreadPool {

        let rooms = Vec::with_capacity(size);
        RoomThreadPool { rooms, max_size: size }
    }

    pub fn craete_new_room(&mut self, name: String, blind: usize) {

        assert!(self.rooms.len() < self.max_size);

        let new_room = Room::new(name, blind);
        self.rooms.push(new_room);

        println!("create_new_rooms: rooms len is {}", self.rooms.len());
    }

    // 일단은 vector 순회로 하시죠
    pub fn find_room_by_id() {

    }

    pub fn get_rooms_info(&self) -> Vec<RoomInfo>{

        println!("rooms len is {}", self.rooms.len());
        let mut rooms_info = Vec::new();
        for room in self.rooms.iter() {
            rooms_info.push(room.room_info.clone());
        }

        rooms_info
    }

    
}