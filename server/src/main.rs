// main.rs 가 최상위 모듈이라 다른 rs를 확인 가능하지만, game.rs에서는 player.rs를 인식할 수 없음.
// lib.rs로 묶어주거나, game 폴더 안에 player.rs를 구현하거나...
mod game;
mod player;
use game::Game;
use hyper_util::rt::TokioIo;

use std::convert::Infallible;
use std::net::SocketAddr;

static INDEX: &[u8] = b"<html><body><form action=\"post\" method=\"post\">Name: <input type=\"text\" name=\"name\"><br>Number: <input type=\"text\" name=\"number\"><br><input type=\"submit\"></body></html>";
static MISSING: &[u8] = b"Missing field";
static NOTNUMERIC: &[u8] = b"Number field is not numeric";

use serde_json::{json, Value};

use serde::{Deserialize, Serialize};

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Full, Empty};

use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use tokio::net::TcpListener;

use rusqlite::{params, Connection, OptionalExtension, Result};
const PATH: &str = "./my_db.db3";

use hyper::{Method, StatusCode};
use std::fs;

fn serve_static_file(path: &str) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {

    let login_html: Vec<u8> = fs::read(path)
                .unwrap_or_else(|_| {
                    b"Error Occured".to_vec()
                });
                
    Ok(Response::new(full(Bytes::from(login_html))))
}

// Infallable 은 절대 실패할 수 없다는 열거형 값이래요
async fn serve_function(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {

    //GET / POST 차이 == 캐싱 여부 & 보안
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
        (&Method::GET, "/game") => serve_static_file("../static/gmae.html"),

        _ => {
            println!("API NOT FOUND");
            let mut not_found = Response::new(Full::new(Bytes::new()));
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(Response::new(not_found.boxed()))
        },
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = TcpListener::bind(addr).await?;

    create_db()?;

    loop {
        let (stream, _) = listener.accept().await?;

        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(serve_function))
                .await
            {
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


