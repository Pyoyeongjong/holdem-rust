// main.rs 가 최상위 모듈이라 다른 rs를 확인 가능하지만, game.rs에서는 player.rs를 인식할 수 없음.
// lib.rs로 묶어주거나, game 폴더 안에 player.rs를 구현하거나...
mod game;
mod player;
use game::Game;
use hyper_util::rt::TokioIo;

use std::convert::Infallible;
use std::net::SocketAddr;

use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use tokio::net::TcpListener;
use http_body_util::Empty;

use hyper::body::Frame;
use hyper::{Method, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt};

use std::fs;

// Infallable 은 절대 실패할 수 없다는 열거형 값이래요
async fn login_page(_: Request<impl hyper::body::Body>) -> Result<Response<Full<Bytes>>, Infallible> {
    
    let login_html: Vec<u8> = fs::read("../static/login.html")
        .unwrap_or_else(|e| {
            b"Error Occured".to_vec()
        });
        
    Ok(Response::new(Full::new(Bytes::from(login_html))))
}

async fn echo(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => Ok(Response::new(full(
            "Try POSTing data to /echo",
        ))),
        (&Method::POST, "/echo") => {
            Ok(Response::new(req.into_body().boxed()))
        },
        (&Method::POST, "/echo/uppercase") => {
            let frame_stream = req.into_body().map_frame(|frame| {
                let frame = if let Ok(data) = frame.into_data() {
                    // Convert every byte in every Data frame to uppercase
                    data.iter()
                        .map(|byte| byte.to_ascii_uppercase())
                        .collect::<Bytes>()
                } else {
                    Bytes::new()
                };
        
                Frame::data(frame)
            });
        
            Ok(Response::new(frame_stream.boxed()))
        }

        _ => {
            let mut not_found = Response::new(empty());
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}



#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;

        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(login_page))
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


