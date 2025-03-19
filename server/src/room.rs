use crate::{game::Game, player::Player};

use std::{
    collections::HashMap, 
    net::SocketAddr,
    sync::{Arc, Mutex},
    sync::atomic::{AtomicUsize, Ordering},
};

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

use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future, pin_mut, stream::TryStreamExt, StreamExt};

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

async fn handle_connection(
    peer_map: PeerMap, 
    ws_stream: WebSocketStream<TokioIo<Upgraded>>,
    addr: SocketAddr,
) {
    println!("Websocket connection established: {}", addr);
    let (tx, rx) = unbounded();
    peer_map.lock().unwrap().insert(addr, tx);

    // outgoing - 서버가 쓰기 전용
    // incoming - 서버가 읽기 전용
    let (outgoing, incoming) = ws_stream.split();

    // Future 생성
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
            }
        }

        future::ok(())
    });

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
    peer_map.lock().unwrap().remove(&addr);
}

pub struct Room {
    pub id: usize,
    game: Game,
    players: Vec<Player>,
    peer_map: PeerMap,
}

impl Room {

    // 방 생성 -> 참여자가 있어야겠죠?
    pub fn new(blind: u32) -> Room {

        let room_idx = ROOMS_IDX.fetch_add(1, Ordering::Relaxed);
        let new_room = Room {
            id: room_idx,
            game: Game::new(blind),
            players: Vec::<Player>::new(),
            peer_map: PeerMap::new(Mutex::new(HashMap::new()))
        };

        new_room
    }

    // 방 삭제
    pub fn remove() {

    }

    // 게임 진행 중에는 관전자로 참여하게 한다.
    // 게임 시작 전에는 참여자로 참가할 수 있어야 한다.

    pub fn add_player(&mut self, player: Player, mut req: Request<Incoming>) {

        // 웹소켓 연결
        let state = self.peer_map.clone();
        tokio::spawn(async move {
            match hyper::upgrade::on(&mut req).await {
                Ok(upgraded) => {
                    let upgraded = TokioIo::new(upgraded);
                    handle_connection(
                        state,
                        WebSocketStream::from_raw_socket(upgraded, Role::Server, None).await,
                        player.addr.unwrap(),
                    )
                    .await;
                },
                Err(e) => println!("upgrade error: {}", e),
            }
        });
    }

    // 나가기 예약, 종료로 인한 플레이어 추방
    pub fn remove_player(&mut self, player: Player) {
        let state = self.peer_map.clone();
    }

    pub fn can_add_player(&self) -> bool { self.players_len() < MAX_PLAYER }
    pub fn players_len(&self) -> usize { self.players.len() }

}

fn show_rooms_info(rooms: Arc<Mutex<Vec<Room>>>) {

}

// 라이프타임: 반환값이 적어도 'a가 붙은 것 중 가장 작은것 동안 유효하다!
pub fn find_room<'a>(id: usize, rooms:&'a mut Vec<Room>) -> Option<&'a mut Room> {
    // 설탕 달달하네..
    rooms.iter_mut().find(|room| room.id == id)
}


