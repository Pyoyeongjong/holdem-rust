// main.rs 가 최상위 모듈이라 다른 rs를 확인 가능하지만, game.rs에서는 player.rs를 인식할 수 없음.
// lib.rs로 묶어주거나, game 폴더 안에 player.rs를 구현하거나...
// 프로젝트 내부 모듈
mod game;
mod player;
use game::Game;

use tokio::net::{TcpStream, TcpListener};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::body::Incoming;
use hyper::upgrade::Upgraded;
use hyper::{header, Method, Request, Response, StatusCode, Version};
// 원래는 http::header인데 re-export를 통해 간단하게 하기 위함
use hyper::header::{
    HeaderValue, CONNECTION, SEC_WEBSOCKET_ACCEPT, SEC_WEBSOCKET_KEY,
    SEC_WEBSOCKET_VERSION, UPGRADE,
};
use hyper_util::rt::TokioIo;
use http_body_util::{combinators::BoxBody, BodyExt, Full, Empty};
use bytes::Bytes;

use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future, pin_mut, stream::TryStreamExt, StreamExt};

use std::{
    convert::Infallible,
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    fs,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use rusqlite::{params, Connection, OptionalExtension, Result};

const PATH: &str = "./my_db.db3";
static INDEX: &[u8] = b"<html><body><form action=\"post\" method=\"post\">Name: <input type=\"text\" name=\"name\"><br>Number: <input type=\"text\" name=\"number\"><br><input type=\"submit\"></body></html>";
static MISSING: &[u8] = b"Missing field";
static NOTNUMERIC: &[u8] = b"Number field is not numeric";

fn serve_static_file(path: &str) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {

    let login_html: Vec<u8> = fs::read(path)
                .unwrap_or_else(|_| {
                    b"Error Occured".to_vec()
                });
                
    Ok(Response::new(full(Bytes::from(login_html))))
}

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
                        println!("Unknown message type");
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

// Infallable 은 절대 실패할 수 없다는 열거형 값이래요
async fn handle_request(
    peer_map: PeerMap,
    mut req: Request<Incoming>,
    addr: SocketAddr,
) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {

    println!("The request's path is: {}", req.uri().path());
    // println!("The request's headers are:");

    // for (ref header, _value) in req.headers() {
    //     println!("* {}", header);
    // }

    let upgrade = HeaderValue::from_static("Upgrade");
    let websocket = HeaderValue::from_static("websocket");

    // 웹소켓 보안 인증 절차
    let headers = req.headers();
    let key = headers.get(SEC_WEBSOCKET_KEY);
    // 클라이언트가 보낸 키를 통해 accept_key를 생성
    let derived = 
        key.map(|k| derive_accept_key(k.as_bytes()));

    // 웹소켓 연결인가 검증하는 과정
    if req.method() == Method::GET
        // && req.version() < Version::HTTP_11
        // && headers
        //     .get(CONNECTION)
        //     .and_then(|h| h.to_str().ok())
        //     .map(|h| {
        //         // http connection 헤더는 공백 또는 쉼표로 여러 값을 가질 수 있기 때문에 나눠서
        //         h.split(|c| c == ' ' || c == ',')
        //         // upgrade랑 비교해서 어떤거든 하나만 있으면 넘기고
        //             .any(|p| p.eq_ignore_ascii_case(upgrade.to_str().unwrap()))
        //     })
        //     .unwrap_or(false)
        // && headers
        //     .get(UPGRADE)
        //     .and_then(|h| h.to_str().ok())
        //     // UPGRADE 헤더는 값이 하나밖에 없기 때문에 split 안하고
        //     .map(|h| h.eq_ignore_ascii_case("websocket"))
        //     .unwrap_or(false)
        // && headers.get(SEC_WEBSOCKET_VERSION).map(|h| h == "13")
        //     .unwrap_or(false)
        // && !key.is_none()
        && req.uri().path() == "/socket"
    {
        // 웹소켓 연결
        let ver = req.version();
        tokio::spawn(async move {
            match hyper::upgrade::on(&mut req).await {
                Ok(upgraded) => {
                    let upgraded = TokioIo::new(upgraded);
                    handle_connection(
                        peer_map,
                        WebSocketStream::from_raw_socket(upgraded, Role::Server, None).await,
                        addr,
                    )
                    .await;
                },
                Err(e) => println!("upgrade error: {}", e),
            }
        });

        // handshake OK sign을 보내야함
        let mut res = Response::new(empty());

        *res.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
        *res.version_mut() = ver;
        res.headers_mut().append(CONNECTION, upgrade);
        res.headers_mut().append(UPGRADE, websocket);
        res.headers_mut().append(SEC_WEBSOCKET_ACCEPT, derived.unwrap().parse().unwrap());
        // Let's add an additional header to our response to the client.
        res.headers_mut().append("MyCustomHeader", ":)".parse().unwrap());
        res.headers_mut().append("SOME_TUNGSTENITE_HEADER", "header_value".parse().unwrap());
        
        Ok(res)

    } else {
        // GET / POST 차이 == 캐싱 여부 & 보안
        // Websocket 제외 일반 통신
        match (req.method(), req.uri().path()) {

            (&Method::GET, "/api/lobby/get-rooms-info") => {
                Ok(Response::new(empty()))
            },

            (&Method::GET, "/api/lobby/get-player-chips") => {
                Ok(Response::new(empty()))
            },

            (&Method::POST, "/api/lobby/create-rooms") => {
                Ok(Response::new(empty()))
            }

            (&Method::POST, "/api/register") => {
                let b = req.collect().await?.to_bytes();

                let params: Value = match serde_json::from_slice(&b) {
                    Ok(val) => val,
                    Err(_) => {
                        return Ok(Response::builder()
                        .status(400)
                        .body(full(MISSING))
                        .unwrap());
                    }
                };
                // JSON을 그대로 to_string 하면 JSON 문자열이 쌍따옴표 붙은 상태로 그대로 string 됨
                let new_user = User {
                    id: params["id"].as_str().unwrap().to_string(),
                    pw: params["pw"].as_str().unwrap().to_string(),
                    email: params["email"].as_str().unwrap_or("").to_string(),
                    chips: 1000
                };

                match save_user(new_user) {
                    Ok(()) => {
                        return Ok(Response::builder()
                        .status(200)
                        .body(full(json!({
                            "success": true,
                        }).to_string()))
                        .unwrap());
                    },
                    Err(_) => {
                        return Ok(Response::builder()
                        .status(400)
                        .body(full(json!({
                            "success": false,
                        }).to_string()))
                        .unwrap());
                    }
                }
            }, 

            (&Method::POST, "/api/check-username") => { // JSON parsing Document Advanced에 있었다. 잘 찾아봐라

                let b = req.collect().await?.to_bytes();

                let params: Value = match serde_json::from_slice(&b) {
                    Ok(val) => val,
                    Err(_) => {
                        return Ok(Response::builder()
                        .status(400)
                        .body(full(MISSING))
                        .unwrap());
                    }
                };

                let id = params["id"].as_str().unwrap_or("").to_string();
                let availalble = match is_user_exist(&id) {
                    Ok(true) => false,
                    _ => true
                };

                let res = json!({
                    "available": availalble
                });

                Ok(Response::new(full(res.to_string())))
            },

            (&Method::POST, "/api/login") => {

                let b = req.collect().await?.to_bytes();

                let params: Value = match serde_json::from_slice(&b) {
                    Ok(val) => val,
                    Err(_) => {
                        return Ok(Response::builder()
                        .status(400)
                        .body(full(MISSING))
                        .unwrap());
                    }
                };

                
                let id = params["id"].as_str().unwrap_or_default().to_string();
                let pw = params["pw"].as_str().unwrap_or_default().to_string();

                let res: Value;
                match find_user(&id, &pw) {
                    Ok(Some(_)) => {
                        res = json!({
                            "success": true,
                            "access_token": "1234"
                        });
                    },
                    Ok(None) => {

                        res = json!({
                            "success": false,
                        });
                        println!("There is no user you are finding.");
                    },
                    Err(_) => {
                        res = json!({
                            "success": false,
                        });
                        println!("Server Error");
                    }
                };

                Ok(Response::new(full(res.to_string())))
            },

            (&Method::GET, "/login") => serve_static_file("../static/login.html"),
            (&Method::GET, "/register") => serve_static_file("../static/register.html"),
            (&Method::GET, "/lobby") => serve_static_file("../static/lobby.html"),
            (&Method::GET, "/game") => serve_static_file("../static/game.html"),
            (&Method::GET, "/favicon.ico") => {
                return Ok(Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(full("NOT FOUND"))
                    .unwrap());
            }
            _ => {
                println!("API NOT FOUND");
                let mut not_found = Response::new(Full::new(Bytes::new()));
                *not_found.status_mut() = StatusCode::NOT_FOUND;
                Ok(Response::new(not_found.boxed()))
            },
        }
    }
}

// 빈 응답문 BoxBody를 생성할 때 사용
fn empty() -> BoxBody<Bytes, Infallible> {
    Empty::<Bytes>::new().boxed()
}

// 일반적인 응답문 BoxBody를 생성할 때 사용
fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, Infallible> {
    Full::new(chunk.into()).boxed()
}

#[derive(Debug, Serialize, Deserialize)]
struct User {
    id: String,
    pw: String,
    email: String,
    chips: u32,
}

fn create_db() -> Result<()> {
    let path = PATH;
    let conn = Connection::open(path)?;

    // PRIMARY KEY, 맨 마지막엔 쉼표 없어야함
    conn.execute(
        "CREATE TABLE IF NOT EXISTS user (
            id      TEXT    PRIMARY KEY,
            pw      TEXT    NOT NULL,
            email   TEXT    NOT NULL,
            chips   INTEGER NOT NULL
        )",
         (),
    )?;
    Ok(())
}

fn save_user(user: User) -> Result<(), rusqlite::Error> {
    let path = PATH;
    let conn = Connection::open(path)?;

    if is_user_exist(&user.id)? {
        println!("User {} is already exist!", &user.id);
        return Ok(());
    }
    
    conn.execute(
        "INSERT INTO user (id, pw, email, chips) VALUES (?1, ?2, ?3, ?4)",
        (&user.id, &user.pw, &user.email, &user.chips),
    )?;

    Ok(())
}

fn is_user_exist(id: &String) -> Result<bool, rusqlite::Error> {

    let path = PATH;
    let conn = Connection::open(path)?;

    // ? 없으면 실행이 안되네
    let mut stmt = conn.prepare(
        "SELECT 1 FROM user WHERE id = ?1 LIMIT 1")?; // 이건 준비일 뿐이고

    let count: Option<i32> = stmt.query_row(params![id], |row| row.get(0))
        .optional()?;

    let res = count.unwrap_or(0) > 0;
    Ok(res)
}

// Option을 잘 사용하는게 중요하다! - 없을 수도 있는
fn find_user(id: &String, pw: &String) -> Result<Option<User>, rusqlite::Error> {

    let path = PATH;
    let conn = Connection::open(path)?;

    let mut stmt = conn.prepare(
        "SELECT id, pw, chips, email FROM user WHERE id = ?1 AND pw = ?2"
    )?;

    println!("{} {}", id, pw);
    // 클로저 사용법 중요한 예시인듯
    let user = stmt.query_row(params![id, pw], |row| {
        Ok(User {
            id: row.get(0)?,
            pw: row.get(1)?,
            chips: row.get(2)?,
            email: row.get(3)?,
        })
    }).optional();

    user
}

use tokio_tungstenite::{
    tungstenite::{
        handshake::derive_accept_key,
        protocol::{Message, Role},
    },
    WebSocketStream,
};

type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;
type Tx = UnboundedSender<Message>;
type Body = http_body_util::Full<hyper::body::Bytes>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let state = PeerMap::new(Mutex::new(HashMap::new()));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = TcpListener::bind(addr).await?;

    create_db()?;

    loop {
        let (stream, addr) = listener.accept().await?;
        let state = state.clone();

        // tokio::task::spawn은 tokio::spawn을 재포장한 것일 뿐이라 기능적으로 동일함
        tokio::spawn(async move {
            // hyper는 기본적으로 tokio::net::TcpStream을 바로 사용할 수 없어서
            // 호환성을 입히기 위해 TokioIo를 사용
            let io = TokioIo::new(stream);
            let service = service_fn(move |req| handle_request(state.clone(), req, addr));
            // with_upgrades를 통해 기본 http 통신을 다른 프로토콜로 덮는다 생각하자
            let conn = http1::Builder::new()
                .serve_connection(io, service)
                .with_upgrades();

            if let Err(err) = conn.await{
                eprintln!("Error serving connection: {:?}", err);
            }
        });        
    }
}

// let mut game = Game::new(10);
// game.insert_player("Steve".to_string(), 1000);
// game.insert_player("Peter".to_string(), 1000);
// game.insert_player("ByungHyeok".to_string(), 1000);

// loop {
//     game.game_start();
// }

/* 다음 해야할 것들

    홀덤 게임
    게임 api를 먼저 구현해야 할 듯 합니다

    웹소켓 연결 카피코딩 했는데, 잘 동작하는지 보려면 좀 봐야할 듯???
    
    2  로비 api 만들기 ( 방 생성, 방 정보 가져오기, 회원 칩 개수 )
    3. 게임 수에 따라서 로비에 방 개수가 뜨게 만들기
    4. 로비 및 게임은 인증한 유저만 들어갈 수 있도록 만들기

    5. API 라우팅 / 모듈화 분리 (크다 싶으면 언제든지 고고)

    6. 비밀번호 해쉬화 ( 보안 문제 )
    7. 로비 및 게임은 인증한 유저만 들어갈 수 있도록 만들기
    
    [ 게임 내 로직 구현하기 ] - 웹소켓 이용하기

    1. 랜덤 입장 기능 구현 
    2. 조건에 맞는 방 찾기(BB 제한, 현재 인원 수, 게임 번호 등)
    2. 게임에 관전 waiting queue 구현 (최대 관전자 수 몰루)
    
    이러한 것들을 관할하는 Lobby 구현하기


*/ 


