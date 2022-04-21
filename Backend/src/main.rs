use futures_util::SinkExt;
use futures_util::StreamExt;
use parking_lot::RwLock;
use poem::{
    get, handler,
    listener::TcpListener,
    middleware::AddData,
    post,
    web::{
        websocket::{Message, WebSocket},
        Data, Multipart,
    },
    EndpointExt, IntoResponse, Route, Server,
};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    collections::HashMap,
    process::Child,
    sync::{atomic::AtomicBool, atomic::Ordering, Arc},
    time::Duration,
};
use tokio::sync::broadcast::Sender;
use tokio::time::sleep;
mod lib;


#[handler]
async fn upload(
    mut multipart: Multipart,
    installing_tasks: Data<&Arc<RwLock<HashMap<String, Child>>>>,
    currently_installing_projects: Data<&Arc<AtomicBool>>,
    sender: Data<&tokio::sync::broadcast::Sender<String>>,
) -> String {
    match lib::upload(multipart, installing_tasks, currently_installing_projects, sender).await {
        Ok(message) => message,
        Err(err) => err.to_string(),
    }
}

#[handler]
pub async fn ws(
    ws: WebSocket,
    clients: Data<&Arc<RwLock<HashMap<String, Sender<String>>>>>,
    information_thread_running: Data<&Arc<AtomicBool>>,
    sender: Data<&tokio::sync::broadcast::Sender<String>>,
) -> impl IntoResponse {
    let tx = sender.clone();
    let tx_2 = sender.clone();
    let mut rx = sender.subscribe();

    let clients = Arc::clone(&clients);
    let tokio_information_thread_clients = Arc::clone(&clients);
    let information_thread_running = Arc::clone(&information_thread_running);
    ws.on_upgrade(move |socket| async move {
        //let (tx, mut rx) = tokio::sync::watch::channel(String::from("channel"));
        let (mut sink, mut stream) = socket.split();
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros()
            .to_string();
        let id_rx = id.clone();
        //let id_tx = id.clone();
        {
            let mut clients_guard = clients.write();
            clients_guard.insert(id, tx);
        }
        //websocket sender
        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                if let Message::Text(rec) = msg {
                    println!("WEBSOCKET: Received message: [{}], [{}]", rec, id_rx);
                    //websocket broadcast
                    if tx_2.send(format!("BROADCAST: [{}] | FROM: [{}]", rec, id_rx)).is_err() {
                        break;
                    }
                }
            }
            let mut clients_guard = clients.write();
            clients_guard.remove(&id_rx);
            println!("WEBSOCKET: SENDER DISCONNECTED: [{}]", id_rx);
        });
        //websocket receiver
        tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if sink.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
            //println!("WEBSOCKET: RECEIVER DISCONNECTED: [{}]", id_tx);
        });

        //run information thread
        if !information_thread_running.load(Ordering::SeqCst) {
            let tokio_information_thread_running = information_thread_running.clone();
            tokio::spawn(async move {
                loop {
                    {
                        let guard = tokio_information_thread_clients.read();
                        if guard.len() < 1 {
                            tokio_information_thread_running.store(false, Ordering::SeqCst);
                            println!("INFORMATION THREAD: Terminating!");
                            break;
                        }
                        println!("INFORMATION THREAD: Running!");
                        for (id, tx) in guard.iter() {
                            match tx.send(String::from("INFORMATION THREAD")) {
                                Ok(_) => {}
                                Err(e) => {
                                    println!(
                                        "ERROR: INFORMATION THREAD: failed to send message [{}]:\n{:?}",
                                        id, e
                                    );
                                }
                            }
                        }
                    }
                    sleep(Duration::from_secs(3)).await;
                }
            });
            information_thread_running.store(true, Ordering::SeqCst);
        }
        else{
            println!(" INFORMATION THREAD: Already running!");
        }
    })
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    //create projects dir
    tokio::fs::create_dir_all(lib::PROJECTS_DIR).await.unwrap();
    //create environments dir
    tokio::fs::create_dir_all(lib::ENVIRONMENTS_DIR)
        .await
        .unwrap();
    //installing tasks
    let installing_tasks: Arc<RwLock<HashMap<String, Child>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let currently_installing_projects = Arc::new(AtomicBool::new(false));

    //clients
    let clients: Arc<RwLock<HashMap<String, Sender<std::string::String>>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let information_thread_running = Arc::new(AtomicBool::new(false));
    
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "poem=debug");
    }
    tracing_subscriber::fmt::init();

    let app = Route::new()
        .at("/upload", post(upload.data(currently_installing_projects)))
        .at("/ws", get(ws.data(information_thread_running)))
        .with(AddData::new(tokio::sync::broadcast::channel::<String>(32).0))
        .with(AddData::new(installing_tasks))
        .with(AddData::new(clients));
        
    Server::new(TcpListener::bind("127.0.0.1:3000"))
        .run(app)
        .await
}