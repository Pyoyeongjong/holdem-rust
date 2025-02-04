use std::net::*;
use std::io::Write;

const SERVER_IP: &str = "127.0.0.1:8080";

fn main() {

    let mut stream = TcpStream::connect(SERVER_IP)
        .expect("Connect error");

    stream.write("A".as_bytes())
        .expect("Transfer error");

    stream.write("B".as_bytes())
        .expect("Transfer error");

    println!("Client End");
}