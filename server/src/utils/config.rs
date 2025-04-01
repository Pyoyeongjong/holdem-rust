use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use futures_channel::mpsc::UnboundedSender;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;

use crate::room::types::TxInfo;

pub const MAX_PLAYER: usize = 6;
pub const SERVER_SECRET: &str = "734c61eebdb501f08ced87f8173ea616e12e9c57036764c71e14f4bc1caf1070";

pub type Tx = UnboundedSender<Message>;
pub type PeerMap = Arc<Mutex<HashMap<SocketAddr, TxInfo>>>;