use jsonwebtoken::{decode, Algorithm, Validation, DecodingKey};

use crate::webcommu::Claims;

// 아이디 반환
pub fn verify_token(token: &str, server_key: &str) -> Option<String> {
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
