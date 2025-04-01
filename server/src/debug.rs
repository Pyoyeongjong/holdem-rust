/* Debug functions! */
/* 그리고 여러 함수들 (공부용) */
#[allow(dead_code)]
fn make_cards() -> Vec<String>{

    let mut deck = Vec::new();
    let mut rng = rand::rng();
    
    let suits = vec!["♠", "◆", "♥", "♣"];
    let ranks = vec!["2", "3", "4", "5", "6", "7", "8", "9", "T", "J", "Q", "K", "A"];
    for suit in suits.iter() {
        for rank in ranks.iter() {
            deck.push(format!("{}{}", suit, rank)); // format은 참조자를 이용한다. -> 그리고 새로운 String 반환환다.
        }
    };

    // for rank in ranks -> 소유권을 가져감
    // for rank in ranks.iter() -> 참조만 함

    deck.shuffle(&mut rng);

    let mut deck: VecDeque<String> = VecDeque::from(deck);
    let mut cards: Vec<String> = Vec::new();
    for _ in 0..7 {
        cards.push(deck.pop_front().unwrap());
    }
    cards
    
}

// // 이 함수가 안되는 이유
// pub async fn handle_connection2(
//     peer_map: PeerMap,
//     ws_stream: WebSocketStream<TokioIo<Upgraded>>,
//     addr: SocketAddr,
//     rooms_thread_pool: Arc<RwLock<RoomThreadPool>>,
// ) {
//     println!("Websocket connection established: {}", addr);
//     let (tx, rx) = unbounded();
    

//     let (outgoing, incoming) = ws_stream.split();

//     // 클로저 바깥 변수를 async move 클로저 안에서 사용하려고 했는데,
//     // 클로저가 move를 요구하면서 소유권 채로 가져가려면
//     // 그 변수는 copy가 가능하거나 소유권 이전이 허용되는 타입이어야 함
//     // 근데 그냥 move하면 바깥 코드에선 더이상 못 쓰니까 Rust가 의도적으로 막는다! (비록 바깥 코드에서 필요없다는 걸 개발자가 알아도, 컴파일러는 명시적으로 행동하게 함)
//     // 왜? 클로저는 내부 실행 시점이 미정이기 때문에
//     let ws_service = incoming.try_for_each(|msg| async move {

//         if let Ok(text) = msg.to_text() {
//             if let Ok(json) = serde_json::from_str::<Value>(text) {
//                 println!("I got this api! {}", json["type"].as_str().unwrap());

//                 match json["type"].as_str() {
//                     Some("join-game") => handle_game(&json, peer_map.clone(), addr, rooms_thread_pool.clone()).await,
//                     _ => {
//                         handle_default(&json).await;
//                     },
//                 }
//             }
//         }
//         future::ok(())
//     });
//     let ws_result_forward= rx.map(Ok).forward(outgoing);
//     // peer_map.lock().unwrap().insert(addr, tx);
//     pin_mut!(ws_service, ws_result_forward);
//     future::select(ws_service, ws_result_forward).await;

//     println!("{} disconnected", &addr);
// }

// unsafe!! 권장되지 않음
// static mut ROOMS: Arc<Mutex<Vec<Room>>> = Arc::new(Mutex::new(Vec::new()));