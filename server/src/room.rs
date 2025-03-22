use std::{
    collections::HashMap, 
    net::SocketAddr,
    sync::{atomic::{AtomicUsize, Ordering}, Arc},
};

use tokio::sync::{Mutex, RwLock};

use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_tungstenite::{
    tungstenite::protocol::{Message, Role},
    WebSocketStream,
};

const SERVER_SECRET: &str = "734c61eebdb501f08ced87f8173ea616e12e9c57036764c71e14f4bc1caf1070";

use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;

use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future::{self}, pin_mut, stream::TryStreamExt, StreamExt};

use serde_json::Value;

use crate::{authentication::verify_token, db::{self, User}, game::Game, player::{self, Player}};

type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

const MAX_PLAYER: usize = 6;
// 전역 변수를 안전하게 접근하고 싶음 이렇게 하자
static ROOMS_IDX: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}


#[derive(Serialize, Deserialize)]
pub struct RoomInfo {
    id: usize,
    name: String,
    max_player: usize,
    cur_player: usize,
    bb: usize,
}

impl RoomInfo {
    pub fn new(id: usize, name: String, bb: usize) -> RoomInfo {
        RoomInfo { id, name, max_player: MAX_PLAYER, cur_player: 0, bb }
    }
}

impl Clone for RoomInfo {
    fn clone(&self) -> Self {
        RoomInfo {
            id: self.id,
            name: self.name.clone(),
            max_player: self.max_player,
            cur_player: self.cur_player,
            bb: self.bb
        }
    }
}

pub struct Room {
    pub id: usize,
    pub room_info: RoomInfo,
    // 이 모든걸 관장하는 쓰레드 (일단 게임은 단일 쓰레드로 하시죠)
    tx_game: mpsc::Sender<GameRequest>,
    rx_room: mpsc::Receiver<GameResponse>,
    // 이 room이 생성한 room thread를 저장해야하는가? 일단 저장해
    game_thread: JoinHandle<()>
}

impl Room {

    pub fn new(name: String, blind: usize) -> Room {

        let room_idx = ROOMS_IDX.fetch_add(1, Ordering::Relaxed);

        let (tx_room, rx_room) = mpsc::channel(32);
        let (tx_game, rx_game) = mpsc::channel(32);

        let game_thread = tokio::spawn(game_thread(rx_game, tx_room.clone()));

        let new_room = Room {
            id: room_idx,
            room_info: RoomInfo::new(room_idx, name, blind),
            tx_game,
            rx_room,
            game_thread
        };

        new_room
    }

    pub async fn add_player(&mut self, new_player: PlayerInfo, tx: UnboundedSender<Message>) -> GameResponse {
        println!("Hi add player!");
        self.tx_game.send(GameRequest::AddPlayer { info: new_player, socket: tx }).await.unwrap();
        let result = self.rx_room.recv().await.unwrap();
        result
    }
}
// 정보 보내기용
pub struct PlayerInfo{
    pub id: String,
    pub name: String,
    pub addr: SocketAddr,
    pub chips: usize,
}

enum GameRequest {
    Command { cmd: GameCommand, id: String},
    AddPlayer { info: PlayerInfo, socket: UnboundedSender<Message>}
}

enum GameCommand {
    StartGame, 
}

pub enum GameResponse {
    Text(String),
}

pub async fn handle_connection(
    peer_map: PeerMap,
    ws_stream: WebSocketStream<TokioIo<Upgraded>>,
    addr: SocketAddr,
    rooms_thread_pool: Arc<RwLock<RoomThreadPool>>,
) {
    //
    let peer_map_cloned = peer_map.clone();
    let rooms_thread_pool_cloned = rooms_thread_pool.clone();
    println!("Websocket connection established: {}", addr);
    let (tx, rx) = unbounded();
    let (outgoing, incoming) = ws_stream.split();
    
    // 매우 어려운 부분
    // 1. async 함수는 async 블록에 있으면 된다
    // 2. move 는 블록 안의 외부 변수들을 블록이 소유권을 가지도록 함
    // 첫 move는 peer_map_cloned, rooms_thread_pool의 소유권을 갖기 위해 사용했고
    // 두번째 move 는 첫 블록에서 생성한 새 clone체 및 외부 변수를 갖기 위해 사용함
    // 이 move 이후 블록 내에서 사용한 외부변수는 블록 바깥에서 사용할 수 없음
    let ws_service = incoming.try_for_each(move |msg| {

        let peer_map_cloned= peer_map_cloned.clone();
        let rooms_thread_pool = rooms_thread_pool_cloned.clone();

        // move를 쓰지 않으면 변수들은 참조로 캡쳐된다.
        // await로 넘긴다는건 -> 이 함수가 멈췄다가 계속 진행할 수 있다.
        // 블록 외부에서 참조를 통해서 변수를 가져오는 건, 블록 외부에서 변수의 생명이 언제 끝날지 모르기 때문에 적절치 않다.
        // 따라서 블록이 직접 변수를 소유하게 해야 한다.
        async move{
            if let Ok(text) = msg.to_text() {
                if let Ok(json) = serde_json::from_str::<Value>(text) {
                    println!("I got this api! {}", json["type"].as_str().unwrap());
    
                    let response: GameResponse = match json["type"].as_str() {
                        // Some("player") => handle_player(json),
                        Some("join-game") => handle_game(&json, peer_map_cloned, addr, rooms_thread_pool).await,
                        // Some("disconnect") => handle_disconnect(json),
                        _ => handle_default(&json).await
                    };
    
                    // 이 ws_service가 room_thread역할 아닌가??
                }
            }
            Ok(())
        }
    });

    // rx에서 받은 메세지를 outgoing으로 보내기
    let ws_result_forward= rx.map(Ok).forward(outgoing);

    peer_map.lock().await.insert(addr, tx);

    // Future
    // return type을 런타임에 알 수 있는 함수

    // 다른 피어로부터 채널로 전달된 메세지를 읽어 webSocket의 쓰기 싱크로 전달한다.
    // map(Ok) => 내용을 Ok로 묶어 forward가 처리할 수 있도록 한다.

    // Future는 한 번 생성되면 메모리 상에서 이동하지 않아야 하므로
    // 매크로를 통해 두 future를 스택에 고정한다.
    pin_mut!(ws_service, ws_result_forward);

    // 두 future를 동시에 실행
    // 하나라도 완료되면 연결 끊어진 것으로 간주
    // future는 polling 해야 실행되는데, await을 통해 자동 폴링이 된다.
    future::select(ws_service, ws_result_forward).await;

    println!("{} disconnected", &addr);
}

// 게임 로직 처리
// 함수에 소유권을 가져왔으니 rx_game에 mut을 붙이든 맘대로 해도 된다
async fn game_thread(mut rx_game: mpsc::Receiver<GameRequest>, tx_room: mpsc::Sender<GameResponse>) {
    
    let mut game = Game::new(100);

    loop {
        let game_request = rx_game.recv().await.unwrap();
        println!("Hi game thread!");
        match game_request {
            GameRequest::Command { cmd, id} => {
                match cmd {
                    GameCommand::StartGame => {

                    }
                }
            },
            GameRequest::AddPlayer { info, socket } => {
                game.insert_player(info, socket);
            },
            _ => {

            }
        }
    }
}

// 이것만 플레이어 연결 없이도 가능하다.
async fn handle_game(json: &Value, peer_map: PeerMap, addr: SocketAddr, rooms_thread_pool: Arc<RwLock<RoomThreadPool>>) -> GameResponse {
    
    let room_id = json["roomId"].as_u64().unwrap() as usize;

    println!("Hello handle_game! room_id={room_id}");
    let access_token = json["access_token"].as_str().unwrap();
    let id = verify_token(access_token, SERVER_SECRET).unwrap();
    // let player_id = authentication::verify_token(access_token, SERVER_SECRET).unwrap();

    // 일단 방 번호로 방을 찾고, 플레이어 id를 통해 사람을 특정시키고
    // 한 줄로 작성하면, lock이 안풀렸는데 &mut 참조를 만들려고 한다. 고 컴파일러가 생각한다.
    // binding을 만들어서, 이게 이 함수 내에서는 끝까지 살아있다를 표현하는 것.
    let mut binding = rooms_thread_pool.write().await;
    let room = binding.find_room_by_id(room_id).unwrap();
    // peermap을 뒤져서 나온 tx를 가져오고
    let player_tx = peer_map.lock().await.get(&addr).unwrap().clone();
    // DB에 id를 통해 플레이어 정보를 가져온다.
    let user = db::find_user_by_id(&id).unwrap().unwrap();

    // DB 상태를 통해 플레이어를 생성
    let new_player = PlayerInfo{ id: user.id.clone(), name: user.id.clone(), chips: user.chips, addr };

    // game_thread가 vec<player> 플레이어의 tx를 저장하도록 한다.

    let result = room.add_player(new_player, player_tx).await;

    result

}

async fn handle_default(json: &Value) -> GameResponse {
    println!("Hello {}", json["type"]);
    let res:GameResponse = GameResponse::Text("abc".to_string());
    res
}


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
    pub fn find_room_by_id(&mut self, id: usize) -> Option<&mut Room> {
        self.find_room(id)
    }

    pub fn get_rooms_info(&self) -> Vec<RoomInfo>{

        println!("rooms len is {}", self.rooms.len());
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

}


