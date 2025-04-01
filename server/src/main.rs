// main.rs 가 최상위 모듈이라 다른 rs를 확인 가능하지만, game.rs에서는 player.rs를 인식할 수 없음.
// lib.rs로 묶어주거나, game 폴더 안에 player.rs를 구현하거나...
// 프로젝트 내부 모듈

// main에 명시해줘야 컴파일 모듈로써 기능을 한다.

use server::game::player;
use server::room::{room_manager::RoomManager, room::handle_connection};
use server::utils::{auth::verify_token, db, http_utils::*, config::PeerMap};

use tokio::net::TcpListener;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};
// 원래는 http::header인데 re-export를 통해 간단하게 하기 위함
use hyper::header::{
    HeaderValue, CONNECTION, SEC_WEBSOCKET_ACCEPT, SEC_WEBSOCKET_KEY, UPGRADE,
};
use hyper_util::rt::TokioIo;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use bytes::Bytes;

use tokio_tungstenite::tungstenite::handshake::derive_accept_key;

use std::{
    convert::Infallible,
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
};

// tokio async 환경에서의 Mutex, RwLock
use tokio::sync::{Mutex, RwLock};

use jsonwebtoken::{encode, Header,  EncodingKey};
use chrono::{Duration, Utc};

use serde_json::{json, Value};

use tokio_tungstenite::{
    tungstenite::protocol::Role,
    WebSocketStream,
};

static MISSING: &[u8] = b"Missing field";
const SERVER_SECRET: &str = "734c61eebdb501f08ced87f8173ea616e12e9c57036764c71e14f4bc1caf1070";

fn send_http_response_falied(code: u16, msg: String) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {
    let res = json!({
        "success": false,
        "msg": msg,
    });
    return Ok(Response::builder()
        .status(code)
        .body(full(res.to_string()))
        .unwrap())
}

// Infallable 은 절대 실패할 수 없다는 열거형 값
async fn handle_request(
    mut req: Request<Incoming>,
    addr: SocketAddr,
    rooms_thread_pool: Arc<RwLock<RoomManager>>, // TODO: 근데 쓰레드 풀 자체가 RwLock일 필요가 있나? Room 별로 RwLock이어야 하지 않나? 생각해보기
    peer_map: PeerMap,
) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {

    println!("Request Occuread at address {}. The request's path is: {}", addr, req.uri().path());

    // 웹소켓 연결이 GET을 사용하는 이유: http 호환성을 위해 하나를 선택해야 되는데, 연결 자체는 변경을 유발하지 않으므로 GET
    if req.method() == Method::GET
        // TODO: 웹소켓 연결인가 검증하는 과정인데, 일단 왜 해야하는지부터 알아야 할 듯
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
        // handle_conection을 통해서 웹소켓을 연결함!

        let upgrade = HeaderValue::from_static("Upgrade");
        let websocket = HeaderValue::from_static("websocket");
    
        // 웹소켓 보안 인증 절차
        let headers = req.headers();
        // Option으로 놔두는건 안좋은듯? 바로바로 처리하기
        let key = match headers.get(SEC_WEBSOCKET_KEY) {
            Some(k) => k,
            None => return send_http_response_falied(400, "Missing SEC_WEBSOCKET_KEY".to_string()),
        };
        
        // 클라이언트가 보낸 키를 통해 accept_key를 생성
        let derived = derive_accept_key(key.as_bytes());
        let ver = req.version();
        tokio::spawn(async move {
            match hyper::upgrade::on(&mut req).await {
                Ok(upgraded) => {
                    let upgraded = TokioIo::new(upgraded);
                    handle_connection(
                        peer_map,
                        WebSocketStream::from_raw_socket(upgraded, Role::Server, None).await,
                        addr,
                        rooms_thread_pool.clone(),
                    ).await;
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
        if let Ok(val) = derived.parse() {
            res.headers_mut().append(SEC_WEBSOCKET_ACCEPT, val);
        }
        // Let's add an additional header to our response to the client.
        res.headers_mut().append("MyCustomHeader", ":)".parse().unwrap());
        res.headers_mut().append("SOME_TUNGSTENITE_HEADER", "header_value".parse().unwrap());
        
        Ok(res)

    } else {
        // GET / POST 차이 == 캐싱 여부 & 보안
        // Websocket 제외 일반 통신
        match (req.method(), req.uri().path()) {
            // POST로는 방만 만들고 입장은 따로 하거나 연계적으로 실시
            (&Method::POST, "/api/lobby/create-room") => {

                let b = req.collect().await?.to_bytes();
                let params: Value = match serde_json::from_slice(&b) {
                    Ok(val) => val,
                    Err(_) => { json!({}) }
                };

                let name = params["name"].as_str().unwrap_or("");
                let blind: usize = params["blind"].as_str().and_then(|b| b.parse::<usize>().ok()).unwrap_or(0);

                let rooms_thread_pool_cloned = rooms_thread_pool.clone();
                let mut rooms_thread_pool = rooms_thread_pool.write().await;

                let success =  rooms_thread_pool.craete_new_room(name, blind, rooms_thread_pool_cloned);

                let res = json!({
                    "success": success,
                });

                Ok(Response::new(full(res.to_string())))
            },

            (&Method::GET, "/api/lobby/get-player-chips") => {

                let header_map = req.headers();
                // expect는 panic 발생용! 보통 unwrap_or을 많이씀
                let access_token = header_map.get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");

                let id = verify_token(access_token, SERVER_SECRET)
                    .unwrap_or("".to_string());

                // 이거 상당히 좋은 방법인듯?
                let (success, chips) = match player::get_player_chips(&id) {
                    Ok(chips) => (true, chips),
                    Err(_) => (false, 0),
                };

                let res = json!({
                    "success": success,
                    "chips": chips
                });
    
                Ok(Response::new(full(res.to_string())))
            },

            (&Method::GET, "/api/lobby/get-rooms-info") => {

                let header_map = req.headers();
                let access_token = header_map.get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");

                let id = verify_token(access_token, SERVER_SECRET).unwrap_or("".to_string());

                if db::find_user_by_id(&id).is_err() {
                    return send_http_response_falied(400, "Id verify Falied".to_string());
                }

                let rooms_thread_pool = rooms_thread_pool.read().await;
                let rooms_info = rooms_thread_pool.get_rooms_info();

                let res = match serde_json::to_string(&rooms_info) {
                    Ok(res) => res,
                    Err(_) => {
                        return send_http_response_falied(400, "Can't get rooms info!".to_string())
                    }
                };
                Ok(Response::new(full(res.to_string())))
            },

            (&Method::POST, "/api/register") => {
                let b = req.collect().await?.to_bytes();

                let params: Value = match serde_json::from_slice(&b) {
                    Ok(val) => val,
                    Err(_) => {
                        return send_http_response_falied(400, "Get Wrong Params!".to_string());
                    }
                };
                
                // JSON을 그대로 to_string 하면 JSON 문자열이 쌍따옴표 붙은 상태로 그대로 string 됨
                let new_user = db::User {
                    id: params["id"].as_str().unwrap_or("").to_string(),
                    pw: params["pw"].as_str().unwrap_or("").to_string(),
                    email: params["email"].as_str().unwrap_or("").to_string(),
                    chips: 1000,
                    refresh_token: None,
                };

                match db::save_user(new_user) {
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
                let availalble = match db::is_user_exist(&id) {
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
                
                let id = params["id"].as_str().unwrap_or("").to_string();
                let pw = params["pw"].as_str().unwrap_or("").to_string();

                let mut res: Value = json!({
                    "success": false,
                });

                match db::find_user(&id, &pw) {

                    Ok(Some(_)) => {
                        let expiration = Utc::now()
                            .checked_add_signed(Duration::seconds(3600))
                            .map(|t| {t.timestamp() as usize})
                            .unwrap();

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let room_thread_pool = Arc::new(RwLock::new(RoomManager::new(5)));
    let peer_map = PeerMap::new(Mutex::new(HashMap::new()));
    let server_addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = TcpListener::bind(server_addr).await?;

    db::create_db()?;

    loop {
        let (stream, addr) = listener.accept().await?;
        let room_thread_pool = room_thread_pool.clone();
        let peer_map = peer_map.clone();

        // tokio::task::spawn은 tokio::spawn을 재포장한 것일 뿐이라 기능적으로 동일함
        tokio::spawn(async move {
            // hyper는 기본적으로 tokio::net::TcpStream을 바로 사용할 수 없어서
            // 호환성을 입히기 위해 TokioIo를 사용
            let io = TokioIo::new(stream);
            let service = service_fn(move |req| handle_request(req, addr, Arc::clone(&room_thread_pool), Arc::clone(&peer_map)));
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


/* 다음 해야할 것들

    [일단 보류]
    url을 통한 의도적인 화면 전환을 막아야할 듯
    뒤로가기를 통해 game화면으로 접근하는 거 막아야할 듯 

    예외처리 싹~ 해주기


    [ 게임 내 추가 로직 구현하기 (생각나는 대로)]
    1. 랜덤 입장 기능 구현 
    2. 조건에 맞는 방 찾기(BB 제한, 현재 인원 수, 게임 번호 등)
    2. 게임에 관전 waiting queue 구현 (최대 관전자 수 몰루)


    디도스 공격을 방어하기 위해 최대 쓰레드 풀을 제한하기

*/ 

