use serde::{Deserialize, Serialize};
use rusqlite::{params, Connection, OptionalExtension, Result};
use bcrypt::{verify, DEFAULT_COST, hash};

const PATH: &str = "./my_db.db3";

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub pw: String,
    pub email: String,
    pub chips: usize,
    pub refresh_token: Option<String>,
}

pub fn create_db() -> Result<()> {
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

// Option을 잘 사용하는게 중요하다! - 없을 수도 있는
pub fn find_user(id: &String, pw: &String) -> Result<Option<User>, rusqlite::Error> {

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

pub fn find_user_by_id(id: &String) -> Result<Option<User>, rusqlite::Error> {

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

pub fn save_user(user: User) -> Result<(), rusqlite::Error> {
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

pub fn is_user_exist(id: &String) -> Result<bool, rusqlite::Error> {

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
