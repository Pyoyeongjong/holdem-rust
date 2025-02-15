/* Logics For Evaluating Hand! */
use core::panic;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum HandRank {
    TopCard(u8, u8, u8, u8, u8),
    Pair(u8, u8, u8, u8),
    TwoPairs(u8, u8, u8),
    Triple(u8, u8, u8),
    Straight(u8),
    Flush(u8, u8, u8, u8, u8),
    FullHouse(u8, u8),
    FourOfCards(u8, u8),
    StraigntFlush(u8, u8, u8, u8, u8),
}

fn evaluate_straight(cards: &Vec<u8>) -> Option<[u8; 5]> {

    if cards.len() < 5 {
        return None
    } 

    let mut prev = cards[0];
    let mut idx = 1;
    let mut count = 1;

    while idx < cards.len() && count < 5 {
        
        let curr = cards[idx];

        if prev - 1 == curr { count += 1; }
        else { count = 1; }

        prev = curr;
        idx += 1;
    }

    if count >= 5 {
        Some([cards[idx-5], cards[idx-4], cards[idx-3], cards[idx-2], cards[idx-1]])
    } else {
        None
    }
}

fn evaluate_most(cards: &mut [u8; 15], cond: u8) -> Option<u8> {

    let mut max: u8 = 0;

    for rank in 2..cards.len() {
        if cards[rank] >= cond {
            max = rank as u8;
        }
    }

    if max == 0 { None } else {
        cards[max as usize] = 0;
        Some(max)
    }  
}

pub fn evaluate_hand(vec: &Vec<String>) -> HandRank {

    let cards = vec;

    let mut suits: [Vec<u8>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()]; // ["♠", "◆", "♥", "♣"]
    let mut ranks: [u8; 15] = [0; 15];
    let mut card_orders: Vec<u8> = Vec::new(); // 중복 없는 내림차순 정렬 (straight 용)
    
    // 정렬 시작
    for card in cards {
        let suit = card.chars().nth(0).unwrap();
        let num = card.chars().nth(1).unwrap();//.to_digit(10).unwrap() as u8;

        let num: u8 = match num {
            'T' => 10,
            'J' => 11,
            'Q' => 12,
            'K' => 13,
            'A' => 14,
            other => other.to_digit(10).unwrap() as u8,
        };

        // 하드코딩 (시간 나면 수정)
        let mut has_same_rank: bool = false;
        for i in 0..card_orders.len() {
            if card_orders[i] as u8 == num {
                has_same_rank = true;
            }
        }
        if !has_same_rank {
            card_orders.push(num); // copy trait을 가진 타입은 복사되고(u8, i32 등), 그렇지 않으면 소유권 이동(&str);
        }

        ranks[num as usize] += 1;

        match suit {
            '♠' => suits[0].push(num),
            '◆' => suits[1].push(num),
            '♥' => suits[2].push(num),
            '♣' => suits[3].push(num),
            _ => panic!("What is this card???"),
        }
    }

    for suit in suits.iter_mut() {
        suit.sort_by(|a, b| b.cmp(a));
    }

    card_orders.sort_by(|a, b| b.cmp(a));

    // print_suits_rank_card_orders(&suits, &ranks, &card_orders);

    // 스티플
    // println!("@@Stifle@@");
    for suit in suits.iter() {
        let straight = evaluate_straight(&suit);
        if straight.is_some() {
            let straight = straight.unwrap();
            return HandRank::StraigntFlush(straight[0], straight[1], straight[2], straight[3], straight[4]);
        }
    }
    
    // 포카드
    // println!("FourCard");
    let four_cards = evaluate_pairs_or_over(vec![4, 1], ranks.clone());
    if four_cards.is_some() {
        let four_cards = four_cards.unwrap();
        return HandRank::FourOfCards(four_cards[0], four_cards[1]);
    }

    // 풀하우스
    // println!("FullHouse");
    let full_house = evaluate_pairs_or_over(vec![3, 2], ranks.clone());
    if full_house.is_some() {
        let full_house = full_house.unwrap();
        return HandRank::FullHouse(full_house[0], full_house[1]);
    }
    
    // 플러시
    // println!("Flush");
    for suit in suits {
        if suit.len() >= 5 {
            return HandRank::Flush(suit[0], suit[1], suit[2], suit[3], suit[4]);
        }
    }

    // 스트레이트
    // println!("Straight");
    let straight = evaluate_straight(&card_orders);
    if straight.is_some() {
        return HandRank::Straight(straight.unwrap()[0]);
    }

    // 트리플
    // println!("Triple");
    let triple = evaluate_pairs_or_over(vec![3, 1, 1], ranks.clone());
    if triple.is_some() {
        let triple = triple.unwrap();
        return HandRank::Triple(triple[0], triple[1], triple[2]);
    }

    // 투페어
    // println!("Twopair");
    let two_pairs = evaluate_pairs_or_over(vec![2, 2, 1], ranks.clone());
    if two_pairs.is_some() {
        let two_pairs = two_pairs.unwrap();
        return HandRank::TwoPairs(two_pairs[0], two_pairs[1], two_pairs[2]);
    }

    // 페어
    // println!("Pair");
    let pair = evaluate_pairs_or_over(vec![2, 1, 1, 1], ranks.clone());
    if pair.is_some() {
        let pair = pair.unwrap();
        return HandRank::Pair(pair[0], pair[1], pair[2], pair[3]);
    }

    // 탑
    // println!("Top");
    let top_card = evaluate_pairs_or_over(vec![1, 1, 1, 1, 1], ranks.clone());
    if top_card.is_some() {
        let top_card = top_card.unwrap();
        return HandRank::TopCard(top_card[0], top_card[1], top_card[2], top_card[3], top_card[4]);
    }

    panic!("Cant reach Here!");
}

fn evaluate_pairs_or_over(cond: Vec<u8>, mut ranks: [u8; 15]) -> Option<Vec<u8>> {
    let mut result: Vec<u8> = Vec::with_capacity(cond.len()); // with_capacity를 해도 인덱스 접근 불가능함!!

    for idx in 0..cond.len() {
        let num = evaluate_most(&mut ranks, cond[idx]);
        if num.is_some() { result.push(num.unwrap()) }
        else { return None }
    }

    Some(result)
}

#[allow(dead_code)]
fn print_suits_rank_card_orders (suits: &[Vec<u8>; 4], ranks: &[u8; 15], card_orders: &Vec<u8>) {
    // print Debug
    print!("Suits:");
    for suit in suits.iter() {
        print!(" [");
        for num in suit.iter() {
            print!("{num} ");
        }
        print!("] ");
    }
    println!("");

    print!("Ranks: [");
    for rank in ranks.iter() {
        print!("{rank} ");
    }
    println!("]");

    print!("Cards: [");
    for card in card_orders.iter() {
        print!("{card} ");
    }
    println!("]");
}
