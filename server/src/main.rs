// main.rs 가 최상위 모듈이라 다른 rs를 확인 가능하지만, game.rs에서는 player.rs를 인식할 수 없음.
// lib.rs로 묶어주거나, game 폴더 안에 player.rs를 구현하거나...
// 프로젝트 내부 모듈
mod game;
mod player;
mod room;
use game::Game;
use room::{Room, find_room};
// use player::make_player_by_http;

use rusqlite::fallible_iterator::Unwrap;
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
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::Message;

use std::{
    convert::Infallible,
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    fs,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use bcrypt::{verify, DEFAULT_COST, hash};

use jsonwebtoken::{encode, decode, Header, Algorithm, Validation, EncodingKey, DecodingKey};
use chrono::{DateTime, Duration, TimeDelta, TimeZone, Utc};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}

use rusqlite::{params, Connection, OptionalExtension, Result};

type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;
type Tx = UnboundedSender<Message>;
type Body = http_body_util::Full<hyper::body::Bytes>;

const PATH: &str = "./my_db.db3";
static INDEX: &[u8] = b"<html><body><form action=\"post\" method=\"post\">Name: <input type=\"text\" name=\"name\"><br>Number: <input type=\"text\" name=\"number\"><br><input type=\"submit\"></body></html>";
static MISSING: &[u8] = b"Missing field";
static NOTNUMERIC: &[u8] = b"Number field is not numeric";
const SERVER_SECRET: &str = "734c61eebdb501f08ced87f8173ea616e12e9c57036764c71e14f4bc1caf1070";


// unsafe!! 권장되지 않음
// static mut ROOMS: Arc<Mutex<Vec<Room>>> = Arc::new(Mutex::new(Vec::new()));

fn serve_static_file(path: &str) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {

    let login_html: Vec<u8> = fs::read(path)
                .unwrap_or_else(|_| {
                    b"Error Occured".to_vec()
                });
                
    Ok(Response::new(full(Bytes::from(login_html))))
}


// Infallable 은 절대 실패할 수 없다는 열거형 값이래요
async fn handle_request(
    peer_map: PeerMap,
    mut req: Request<Incoming>,
    addr: SocketAddr,
    rooms: Arc<Mutex<Vec<Room>>>,
) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {

    println!("The request's path is: {}", req.uri().path());

    let upgrade = HeaderValue::from_static("Upgrade");
    let websocket = HeaderValue::from_static("websocket");

    // 웹소켓 보안 인증 절차
    let headers = req.headers();
    let key = headers.get(SEC_WEBSOCKET_KEY);
    // 클라이언트가 보낸 키를 통해 accept_key를 생성
    let derived = 
        key.map(|k| derive_accept_key(k.as_bytes()));

    // 웹소켓 연결인가 검증하는 과정
    // http 호환성을 위해 하나를 선택해야 되는데, 연결 자체는 변경을 유발하지 않으므로 GET
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
        // let mut rooms = rooms.lock().unwrap();
        
        // // 방 번호에 해당하는 방 찾기
        // let mut room = find_room(id, &mut rooms);
        // if room.is_none() {
        //     // 방이 없음 && 만드려고 했으면 생성
            
        //     // 아니면 에러
        // }

        // let mut room = room.unwrap();
        // let ver = req.version();

        // // 방에 남는 자리 있으면
        // if room.can_add_player() {
        //     // 방을 통해 웹소켓 연결
        //     let player = make_player_by_http(&req);
        //     room.add_player(player, req);
        // } else {
        //     // 방이 꽉 찼다는 에러!!
        //     return ERR
        // }

        // // handshake OK sign을 보내야함
        let mut res = Response::new(empty());

        *res.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
        // *res.version_mut() = ver;
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

            (&Method::GET, "/api/lobby/get-player-chips") => {


                let header_map = req.headers();
                // expect는 panic 발생용! 보통 unwrap_or을 많이씀
                let access_token = header_map.get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .expect("Access Token is not here");

                println!("{access_token}");

                let id = verify_token(access_token, SERVER_SECRET).expect("id verify failed!");

                println!("{id}");

                let chips = get_player_chips(&id).expect("Failed to get chips");
                println!("{chips}");

                let res = json!({
                    "chips": chips
                });
    
                Ok(Response::new(full(res.to_string())))
            },

            (&Method::GET, "/api/lobby/get-rooms-info") => {

                println!("Hello get rooms info!");

                let header_map = req.headers();
                // expect는 panic 발생용! 보통 unwrap_or을 많이씀
                let access_token = header_map.get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .expect("Access Token is not here");

                println!("{access_token}");

                let id = verify_token(access_token, SERVER_SECRET).expect("id verify failed!");

                println!("{id}");

                let res = json!({
                    "rooms": [
                        {"id": 1, "bb": 100, "participants": 1, "max_participants": 6},
                        {"id": 2, "bb": 200, "participants": 3, "max_participants": 6},
                        {"id": 3, "bb": 300, "participants": 5, "max_participants": 6},
                    ]
                });

                Ok(Response::new(full(res.to_string())))
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
                    chips: 1000,
                    refresh_token: None,
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

                let mut res: Value = json!({
                    "success": false,
                });
                match find_user(&id, &pw) {
                    Ok(Some(_)) => {
                        let expiration = Utc::now()
                            .checked_add_signed(Duration::seconds(3600))
                            .expect("valid timestamp")
                            .timestamp() as usize;

                        // token 만들기
                        let claim: Claims = Claims {
                            sub: id,
                            exp: expiration,
                        };

                        // as_ref == Option<T> 를 Option<&T>로 변경
                        let token = encode(&Header::default(), &claim, 
                            &EncodingKey::from_secret(SERVER_SECRET.as_ref())).unwrap();
                            
                            
                        res = json!({
                            "success": true,
                            "access_token": token,
                        });
                    },
                    Ok(None) => {
                        println!("There is no user you are finding.");
                    },
                    Err(_) => {
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
    refresh_token: Option<String>,
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
            chips   INTEGER NOT NULL,
            refresh_token   TEXT
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
    
    let hashed_pw = match hash(&user.pw, DEFAULT_COST) {
        Ok(hp) => hp,
        Err(_) => {
            println!("Failed to Hash password!");
            return Err(rusqlite::Error::InvalidQuery);
        }
    };

    conn.execute(
        "INSERT INTO user (id, pw, email, chips, refresh_token) VALUES (?1, ?2, ?3, ?4, ?5)",
        (&user.id, &hashed_pw, &user.email, &user.chips, &user.refresh_token),
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

fn get_player_chips(id: &String) -> Result<usize, rusqlite::Error> {
    let user = find_user_by_id(id).expect("find_user_by_id: can't find player").expect("222");
    let chips = user.chips as usize;
    Ok(chips)
}

// Option을 잘 사용하는게 중요하다! - 없을 수도 있는
fn find_user(id: &String, pw: &String) -> Result<Option<User>, rusqlite::Error> {

    let path = PATH;
    let conn = Connection::open(path)?;

    let mut stmt = conn.prepare(
        "SELECT id, pw, chips, email, refresh_token FROM user WHERE id = ?1"
    )?;
    
    // 클로저 사용법 중요한 예시인듯
    
    let user = stmt.query_row(params![id], |row| {
        let stored_pw: String = row.get(1)?;
        if verify(pw, &stored_pw).unwrap_or(false) {
            Ok(User {
                id: row.get(0)?,
                pw: stored_pw.clone(),
                chips: row.get(2)?,
                email: row.get(3)?,
                refresh_token: row.get(4)?,
            })
        } else {
            // 에러 처리해서 optional 통과했을 때 Ok(None)이 되도록
            Err(rusqlite::Error::QueryReturnedNoRows)
        }
    }).optional();

    user
}

fn find_user_by_id(id: &String) -> Result<Option<User>, rusqlite::Error> {

    let path = PATH;
    let conn = Connection::open(path)?;

    let mut stmt = conn.prepare(
        "SELECT id, pw, chips, email, refresh_token FROM user WHERE id = ?1"
    )?;
    
    // 클로저 사용법 중요한 예시인듯
    
    let user = stmt.query_row(params![id], |row| {{
        Ok(User {
            id: row.get(0)?,
            pw: row.get(1)?,
            chips: row.get(2)?,
            email: row.get(3)?,
            refresh_token: row.get(4)?,
        })
    }}).optional();

    user
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let rooms: Arc<Mutex<Vec<Room>>> = Arc::new(Mutex::new(Vec::new()));
    let state = PeerMap::new(Mutex::new(HashMap::new()));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = TcpListener::bind(addr).await?;

    create_db()?;

    loop {
        let (stream, addr) = listener.accept().await?;
        let state = state.clone();
        let rooms = rooms.clone();

        // tokio::task::spawn은 tokio::spawn을 재포장한 것일 뿐이라 기능적으로 동일함
        tokio::spawn(async move {
            // hyper는 기본적으로 tokio::net::TcpStream을 바로 사용할 수 없어서
            // 호환성을 입히기 위해 TokioIo를 사용
            let io = TokioIo::new(stream);
            let service = service_fn(move |req| handle_request(state.clone(), req, addr, rooms.clone()));
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

// 아이디 반환
fn verify_token(token: &str, server_key: &str) -> Option<String> {
    let decode_key = DecodingKey::from_secret(server_key.as_ref());
    let token_message = decode::<Claims>(token, &decode_key, &Validation::new(Algorithm::HS256));
    match token_message {
        Ok(token) => Some(token.claims.sub),
        Err(_) => {
            println!("Verify token failed");
            None
        }
    }
}

/* 다음 해야할 것들

    1. 로비 및 게임은 인증한 유저만 들어갈 수 있도록 만들기 (생성한 토큰을 페이지 들어갈 때마다 인증을 해야하는지가 의문이다.. 일단 패스)
        그리고 snowmen_fight에서는 라우팅으로 화면 전환하지 않고 게임씬으로 전환하는데.. 나도 url을 통한 의도적인 화면 전환을 막아야하나 싶음
    
    2  로비 api 만들기 ( 방 생성, 방 정보 가져오기 Ok( 내부적 변형만 하면 됨 ), 회원 칩 개수 Ok ) -- 일부 오케이

    3. 방 생성을 어떻게 해야할까..

    5. API 라우팅 / 모듈화 분리 (크다 싶으면 언제든지 고고)
    
    [ 게임 내 로직 구현하기 ] - 웹소켓 이용하기

    1. 랜덤 입장 기능 구현 
    2. 조건에 맞는 방 찾기(BB 제한, 현재 인원 수, 게임 번호 등)
    2. 게임에 관전 waiting queue 구현 (최대 관전자 수 몰루)
    
    이러한 것들을 관할하는 Lobby 구현하기


*/ 


