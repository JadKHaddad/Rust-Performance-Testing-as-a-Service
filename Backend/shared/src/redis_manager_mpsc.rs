//This is an another version of the redis manager that uses an mpsc channel instead of a broadcast channel.
//For testing reasons.
//Do not use!
use redis::cmd;
use redis::RedisResult;
use redis::Value;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct Manager {
    client: redis::Client,
    connection: Arc<Mutex<redis::Connection>>,
    reconnecting: Arc<Mutex<bool>>,
    tx: tokio::sync::mpsc::Sender<bool>,
    rx: Arc<Mutex<tokio::sync::mpsc::Receiver<bool>>>,
}

/// The Manager acts as connector to the redis server. The Creation of the
/// Manager is blocking until a connection is established.
/// The Manager will automatically - non-blocking - reconnect on all query 
/// failures. Be sure to use a valid query. Tokio compatible.
impl Manager {
    pub async fn new(client: redis::Client) -> Manager {
        let connection;
        loop {
            if let Ok(con) = client.get_connection() {
                connection = con;
                println!("[{}] REDIS MANAGER: Connected!", crate::get_date_and_time());
                break;
            }
            println!("[{}] REDIS MANAGER: Reconnecting!", crate::get_date_and_time());
            sleep(Duration::from_secs(3)).await;
        }
        let (tx, rx) = mpsc::channel::<bool>(100);
        Manager {
            connection: Arc::new(Mutex::new(connection)),
            client: client,
            reconnecting: Arc::new(Mutex::new(false)),
            tx: tx,
            rx: Arc::new(Mutex::new(rx)),
        }
    }

    fn reconnect(&mut self) {
        let mut reconnecting = self.reconnecting.lock().unwrap();
        if *reconnecting {
            return;
        }
        *reconnecting = true;
        let connection = self.connection.clone();
        let client = self.client.clone();
        let reconnecting = self.reconnecting.clone();
        let rx = self.rx.clone();
        tokio::spawn(async move {
            loop {
                if rx.lock().unwrap().try_recv().is_ok(){
                    println!("[{}] REDIS MANAGER: Reconnection thread terminated!", crate::get_date_and_time());
                    break;
                }
                println!("[{}] REDIS MANAGER: Reconnecting!", crate::get_date_and_time());
                if let Ok(mut x) = connection.lock() {
                    if let Ok(connection) = client.get_connection() {
                        *x = connection;
                        println!("[{}] REDIS MANAGER: Reconnected!", crate::get_date_and_time());
                        break;
                    }
                }
                sleep(Duration::from_secs(2)).await;
            }
            let mut reconnecting = reconnecting.lock().unwrap();
            *reconnecting = false;
        });
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            if tx.send(true).await.is_ok(){
                println!("[{}] REDIS MANAGER: Terminaiting Reconnection thread!", crate::get_date_and_time());
            }
        });
        
    }
}

impl redis::ConnectionLike for Manager {
    fn get_db(&self) -> i64 {
        self.client.get_connection_info().redis.db
    }

    fn req_packed_command(&mut self, cmd: &[u8]) -> RedisResult<Value> {
        let connection = self.connection.clone();
        let mut x = connection.lock().unwrap();
        let result = x.req_packed_command(cmd);
        if result.is_err() {
            self.reconnect();
        }
        result
    }

    fn req_packed_commands<'a>(
        &'a mut self,
        cmd: &[u8],
        offset: usize,
        count: usize,
    ) -> RedisResult<Vec<Value>> {
        let connection = self.connection.clone();
        let mut x = connection.lock().unwrap();
        let result = x.req_packed_commands(cmd, offset, count);
        if result.is_err() {
            self.reconnect();
        }
        result
    }

    fn is_open(&self) -> bool {
        let x = self.connection.lock().unwrap();
        x.is_open()
    }

    fn check_connection(&mut self) -> bool {
        cmd("PING").query::<String>(self).is_ok()
    }
}