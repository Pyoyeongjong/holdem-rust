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
    // let file = fs::read(path).unwrap_or_else(|_| b"Error Occured".to_vec());
    // Ok(Response::new(full(Bytes::from(file))))

    let login_html: Vec<u8> = fs::read(path)
                .unwrap_or_else(|e| {
                    b"Error Occured".to_vec()
                });
                
    Ok(Response::new(full(Bytes::from(login_html))))
}

// Infallable 은 절대 실패할 수 없다는 열거형 값이래요
async fn serve_function(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {

    match (req.method(), req.uri().path()) {

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

            let id = params["id"].as_str().unwrap_or_default().to_string();

            let res = json!({
                "available": !find_one(&id).unwrap(),
            });

            Ok(Response::new(full(res.to_string())))
        },

        (&Method::GET, "/login") => serve_static_file("../static/login.html"),
        (&Method::GET, "/register") => serve_static_file("../static/register.html"),
        (&Method::GET, "/lobby") => serve_static_file("../static/lobby.html"),
        (&Method::GET, "/game") => serve_static_file("../static/gmae.html"),

        _ => {
            println!("NOT FOUND");
            let mut not_found = Response::new(Full::new(Bytes::new()));
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(Response::new(not_found.boxed()))
        },
    }
}

fn empty() -> BoxBody<Bytes, Infallible> {
    Empty::<Bytes>::new().boxed()
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, Infallible> {
    Full::new(chunk.into()).boxed()
}

#[derive(Debug)]
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

fn save_user(id: String, pw: String, email: String) -> Result<()> {
    let path = PATH;
    let conn = Connection::open(path)?;

    let me = User {
        id: id,
        pw: pw,
        email: email,
        chips: 1000,
    };

    if find_one(&me.id)? {
        println!("User {} is already exist!", &me.id);
        return Ok(());
    }
    
    conn.execute(
        "INSERT INTO user (id, pw, email, chips) VALUES (?1, ?2, ?3, ?4)",
        (&me.id, &me.pw, &me.email, &me.chips),
    )?;

    Ok(())
}

fn find_one(id: &String) -> Result<bool, rusqlite::Error> {

    let path = PATH;
    let conn = Connection::open(path)?;

    // ? 없으면 실행이 안되네
    let mut stmt = conn.prepare(
        "SELECT 1 FROM user WHERE id = ?1 LIMIT 1")?; // 이건 준비일 뿐이고

    let count: Option<i32> = stmt.query_row(params![id], |row| row.get(0))
        .optional()?;

    Ok(count.unwrap_or(0) > 0)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = TcpListener::bind(addr).await?;

    create_db()?;
    save_user("pyjong1999".to_string(), "asdfg1213".to_string(), "pyjong1999@gmail.com".to_string())?;
    save_user("pyjong1998".to_string(), "asdfg122".to_string(), "pyjong1999@naver.com".to_string())?;
    save_user("pyjong1997".to_string(), "asdfg123".to_string(), "pyjong1999@gmail.com".to_string())?;

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

    방 하나 있는 홀덤 게임 고고
    1. TcpListener로 Html 뿌리기
    2. 로그인 / 회원가입 기능 구현하기


    1. 랜덤 입장 기능 구현 
    2. 조건에 맞는 방 찾기(BB 제한, 현재 인원 수, 게임 번호 등)
    2. 게임에 관전 waiting queue 구현 (최대 관전자 수 몰루)
    
    이러한 것들을 관할하는 Lobby 구현하기


*/ 


