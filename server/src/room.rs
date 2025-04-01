use std::{
    collections::HashMap, 
    net::SocketAddr,
    sync::{atomic::{AtomicUsize, Ordering}, Arc},
};

use tokio::sync::{Mutex, RwLock};

use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_tungstenite::{
    tungstenite::{protocol::Message, Utf8Bytes},
    WebSocketStream,
};

use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;

use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future::{self}, pin_mut, stream::TryStreamExt, StreamExt};

use serde_json::{json, Value};

use crate::{authentication::verify_token, db::{self}, game::Game, params};
type Tx = UnboundedSender<Message>;
pub type PeerMap = Arc<Mutex<HashMap<SocketAddr, TxInfo>>>;

// 전역 변수를 안전하게 접근하고 싶음 이렇게 하자
static ROOMS_IDX: AtomicUsize = AtomicUsize::new(0);

const MAX_PLAYER: usize = params::MAX_PLAYER;
const SERVER_SECRET: &str = params::SERVER_SECRET;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}

pub struct TxInfo {
    pub tx: Tx,
    pub room_id: Option<usize>
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
    _rx_room: mpsc::Receiver<GameResponse>,
    // 이 room이 생성한 room thread를 저장해야하는가? 일단 저장해
    _game_thread: JoinHandle<()>
}

impl Room {

    pub fn new(name: String, blind: usize, rooms_thread_pool: Arc<RwLock<RoomThreadPool>>) -> Room {

        let room_idx = ROOMS_IDX.fetch_add(1, Ordering::Relaxed);

        let (tx_room, rx_room) = mpsc::channel(32);
        let (tx_game, rx_game) = mpsc::channel(32);

        let game_thread = tokio::spawn(game_thread(room_idx, rx_game, tx_room.clone(), rooms_thread_pool));

        let new_room = Room {
            id: room_idx,
            room_info: RoomInfo::new(room_idx, name, blind),
            tx_game,
            _rx_room: rx_room,
            _game_thread: game_thread
        };

        new_room
    }

    // 
    pub async fn add_player(&mut self, new_player: PlayerInfo, tx: UnboundedSender<Message>) {
        println!("Hi add player!");
        self.room_info.cur_player += 1;
        self.tx_game.send(GameRequest::AddPlayer { info: new_player, socket: tx }).await.unwrap();
        // room한테 굳이 보고를 해야하나??
        // let result = self.rx_room.recv().await.unwrap();
        // result
    }

    pub async fn delete_player(&mut self, player_addr: SocketAddr) {
        self.room_info.cur_player -= 1;
        self.tx_game.send(GameRequest::RemovePlayer { addr: player_addr }).await.unwrap();
    }

    pub async fn pass_action(&mut self, cmd: GameCommand, id: String) {
        self.tx_game.send(GameRequest::Command { cmd, id }).await.unwrap();
    }
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
    
                    match json["type"].as_str() {
                        Some("join-game") => handle_join_game(&json, peer_map_cloned, addr, rooms_thread_pool).await,
                        Some("action") => handle_action(&json, rooms_thread_pool).await,
                        // Some("disconnect") => handle_disconnect(&json, peer_map_cloned, addr, rooms_thread_pool).await,
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

    /* RAII + 스코프 기반 자원 관리 개념!
    MutexGuard 리턴 값을 let으로 지정하지 않고 한줄짜리로 쓰면 임시값이 된다.
    let으로 저장하지 않은 값은 사용된 표현식이 끝날 때 drop된다.
    체이닝 메소드는 체인 전체가 끝날 때 drop된다. */
    
    peer_map.lock().await.insert(addr, TxInfo {tx, room_id: None});

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
    // Disconnect 작업

    // 1. room_thread_pool -> room 한테 죽은 걸 알리기 (어떻게 특정한담?) -> peer_map에 기록해놓자!
    // 2. room이 game_thread한테 delete room 알리기
    if let Some(tx_info) = peer_map.lock().await.get(&addr) {
        if let Some(room_id) = tx_info.room_id {
            rooms_thread_pool.write().await.find_room_by_id(room_id).unwrap().delete_player(addr).await;
        }
    }
    // 3. peer_map 정리
}

// 게임 로직 처리
// 함수에 소유권을 가져왔으니 rx_game에 mut을 붙이든 맘대로 해도 된다
async fn game_thread(id: usize, mut rx_game: mpsc::Receiver<GameRequest>, _tx_room: mpsc::Sender<GameResponse>, rooms_thread_pool: Arc<RwLock<RoomThreadPool>>) {
    
    let mut game = Game::new(id, 100);

    loop {
        if game.get_players_len() <= 0 {
            break;
        }

        let game_request = rx_game.recv().await.unwrap();

        println!("Hi game thread!");
        let is_finished = match game_request {
            GameRequest::Command { cmd, id} => {
                match cmd {
                    GameCommand::StartGame => {
                        if game.is_game_can_start() {
                            game.game_start();
                        } else {
                            println!("Game can't start");
                        }
                        false
                    },
                    GameCommand::Check
                    | GameCommand::Call
                    | GameCommand::Raise(_)
                    | GameCommand::AllIn
                    | GameCommand::Fold => {
                        if game.is_game_start() && game.auth_player(id) {
                            game.betting_phase_action(cmd)
                        } else {
                            println!("Game is not started or you are not turn");
                            false
                        }
                    },
                }
            },
            GameRequest::AddPlayer { info, socket } => {
                game.insert_player(info, socket.clone());
                socket.unbounded_send(Message::Text(Utf8Bytes::from(json!({
                    "type": "msg"
                }).to_string()))).unwrap();
                false
            },
            GameRequest::RemovePlayer { addr } => {
                game.delete_player_by_addr(addr);
                false
            }
        };

        // broadcast info
        game.broadcast(is_finished);
        // next Player 에게 state 따로 보내기
        game.betting_phase_annotation();
    }
    // Game 정리

    // room에게 죽으라고 알려주기 가 아니라 너가 직접 정리해
    let mut pool = rooms_thread_pool.write().await;
    pool.delete_room(id);

}

// 이것만 플레이어 연결 없이도 가능하다.
async fn handle_join_game(json: &Value, peer_map: PeerMap, addr: SocketAddr, rooms_thread_pool: Arc<RwLock<RoomThreadPool>>) {
    
    let room_id = json["roomId"].as_u64().unwrap() as usize;
    println!("Hello handle_game! room_id={room_id}");
    let access_token = json["access_token"].as_str().unwrap();
    let player_id = verify_token(access_token, SERVER_SECRET).unwrap();

    // let player_id = authentication::verify_token(access_token, SERVER_SECRET).unwrap();

    // 일단 방 번호로 방을 찾고, 플레이어 id를 통해 사람을 특정시키고
    // 한 줄로 작성하면, lock이 안풀렸는데 &mut 참조를 만들려고 한다. 고 컴파일러가 생각한다.
    // binding을 만들어서, 이게 이 함수 내에서는 끝까지 살아있다를 표현하는 것.
    let mut binding = rooms_thread_pool.write().await;
    let room = binding.find_room_by_id(room_id).unwrap();
    // peermap을 뒤져서 나온 tx를 가져오고
    let player_tx = peer_map.lock().await.get(&addr).unwrap().tx.clone();
    // DB에 id를 통해 플레이어 정보를 가져온다.
    let user = db::find_user_by_id(&player_id).unwrap().unwrap();
    // DB 상태를 통해 플레이어를 생성
    let new_player = PlayerInfo{ _id: user.id.clone(), name: user.id.clone(), chips: user.chips, addr };

    // game_thread가 vec<player> 플레이어의 tx를 저장하도록 한다.
    room.add_player(new_player, player_tx).await;
    // result
    
    if let Some(tx_info) = peer_map.lock().await.get_mut(&addr) {
        tx_info.room_id = Some(room_id);
    }
    
    

}

async fn handle_action(json: &Value, rooms_thread_pool: Arc<RwLock<RoomThreadPool>>) {
    
    let room_id = json["roomId"].as_u64().unwrap() as usize;
    let access_token = json["access_token"].as_str().unwrap();
    let id = verify_token(access_token, SERVER_SECRET).unwrap();

    let action = json["player_action"].as_str().unwrap();
    println!("handle_action: {action}, with {id}");
    let cmd = match action {
        "game_start" => { GameCommand::StartGame },
        "check" => { GameCommand::Check },
        "call" => { GameCommand::Call },
        "raise" => {
            let size = json["size"].as_u64().unwrap() as usize;
            GameCommand::Raise(size)
        },
        "allin" => { GameCommand::AllIn },
        "fold" => { GameCommand::Fold }
        _ => return
    };

    let mut binding = rooms_thread_pool.write().await;
    let room = binding.find_room_by_id(room_id).unwrap();

    room.pass_action(cmd, id).await;
}

async fn handle_default(json: &Value) {
    println!("Hello {}", json["type"]);
}


pub struct RoomThreadPool{
    rooms: Vec<Room>,
    max_size: usize,
}

impl RoomThreadPool {
    pub fn new(size: usize) -> RoomThreadPool {

        let rooms = Vec::with_capacity(size);
        // Game Thread가 종료하면서 room_thread에게 room 청소해달라고 요청할 거임

        RoomThreadPool { rooms, max_size: size }
    }

    pub fn craete_new_room(&mut self, name: &str, blind: usize, room_thread_pool: Arc<RwLock<RoomThreadPool>>) -> bool {

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

// 정보 보내기용
pub struct PlayerInfo{
    pub _id: String,
    pub name: String,
    pub addr: SocketAddr,
    pub chips: usize,
}

enum GameRequest {
    Command { cmd: GameCommand, id: String},
    AddPlayer { info: PlayerInfo, socket: UnboundedSender<Message>},
    RemovePlayer { addr: SocketAddr }
}

pub enum GameCommand {
    StartGame, 
    Check,
    Call,
    Raise(usize),
    AllIn,
    Fold,
}

pub enum GameResponse {}


