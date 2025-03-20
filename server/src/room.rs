use crate::{game::Game, player::Player};

use std::{
    collections::HashMap, 
    net::SocketAddr,
    sync::{atomic::{AtomicUsize, Ordering}, Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_tungstenite::{
    tungstenite::{
        handshake::derive_accept_key,
        protocol::{Message, Role},
    },
    WebSocketStream,
};

use hyper::body::Incoming;
use hyper::{Request, Response};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;

use futures_channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures_util::{future::{self, Join}, pin_mut, stream::TryStreamExt, StreamExt};

use serde_json::Value;

type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

const MAX_PLAYER: usize = 6;
// 전역 변수를 안전하게 접근하고 싶음 이렇게 하자
static ROOMS_IDX: AtomicUsize = AtomicUsize::new(0);

// 한 번에 끝낼 수 있는 작업들은 async를 안붙여도 됨.
fn handle_player(json: Value) {
    println!("Hello handle_player!");
}

fn handle_game(json: Value) {
    println!("Hello handle_game!");
}

fn handle_disconnect(json: Value) {
    println!("Hello handle_disconnect");
}

fn handle_default(json: Value) {
    println!("Hello {}", json["type"]);
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

    pub async fn connect_websocket(&mut self, addr: SocketAddr, mut req: Request<Incoming>) {

        let (tx, rx) = unbounded();
        // 바깥에서 handle_coonection을 통해서 웹소켓을 연결함!
        tokio::spawn(async move {
            match hyper::upgrade::on(&mut req).await {
                Ok(upgraded) => {
                    let upgraded = TokioIo::new(upgraded);
                    handle_connection(
                        WebSocketStream::from_raw_socket(upgraded, Role::Server, None).await,
                        addr,
                        rx
                    ).await;
                },
                Err(e) => println!("upgrade error: {}", e),
            }
        });

        let tx_game = self.tx_game.clone();
        // Player를 생성하고 플레이어 주소와 tx를 작성해서 game thread에 보낸다.
        
        let new_player_info = PlayerInfo {
            id: "pyjong1999".to_string(),
            name: "Baekihwan".to_string(),
            addr: addr,
            chips: 1000 
        };
        tx_game.send(GameRequest::AddPlayer { info: new_player_info, socket: tx }).await.unwrap();
    }

}

async fn handle_connection(
    ws_stream: WebSocketStream<TokioIo<Upgraded>>,
    addr: SocketAddr,
    rx: UnboundedReceiver<Message>
) {
    println!("Websocket connection established: {}", addr);
    // self.peer_map.lock().unwrap().insert(addr, tx);
    // 위 코드 대신 Player에 addr, tx를 insert
    let (outgoing, incoming) = ws_stream.split();

    // Future 생성
    // 이 부분에서 메세지 파싱 후 game_thread에게 보내줘야 함
    // 이 부분에서 완성된 답장을 tx에 해주는 것
    let ws_service = incoming.try_for_each(|msg| {
        if let Ok(text) = msg.to_text() {
            if let Ok(json) = serde_json::from_str::<Value>(text) {
                match json["type"].as_str() {
                    Some("player") => handle_player(json),
                    Some("game") => handle_game(json),
                    Some("disconnect") => handle_disconnect(json),
                    _ => {
                        handle_default(json)
                    },            
                }

                // 이 ws_service가 room_thread역할 아닌가??
            }
        }

        future::ok(())
    });

    // rx에서 받은 메세지를 outgoing으로 보내기
    let ws_result_forward= rx.map(Ok).forward(outgoing);

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

fn show_rooms_info(rooms: Arc<Mutex<Vec<Room>>>) {

}

// 라이프타임: 반환값이 적어도 'a가 붙은 것 중 가장 작은것 동안 유효하다!
pub fn find_room<'a>(id: usize, rooms:&'a mut Vec<Room>) -> Option<&'a mut Room> {
    // 설탕 달달하네..
    rooms.iter_mut().find(|room| room.id == id)
}

// 메세지 Parsing
async fn room_thread(rx_room: mpsc::Receiver<(String)>, tx_game: mpsc::Sender<(String)>) {

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

enum GameResponse {
    String,
}

// 게임 로직 처리
// 함수에 소유권을 가져왔으니 rx_game에 mut을 붙이든 맘대로 해도 된다
async fn game_thread(mut rx_game: mpsc::Receiver<GameRequest>, tx_room: mpsc::Sender<GameResponse>) {
    let mut game = Game::new(100);

    loop {
        let game_request = rx_game.recv().await.unwrap();

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



