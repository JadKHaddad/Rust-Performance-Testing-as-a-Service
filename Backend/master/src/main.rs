use futures_util::SinkExt;
use futures_util::StreamExt;
use parking_lot::RwLock;
use poem::{
    endpoint::StaticFilesEndpoint,
    get, handler,
    listener::TcpListener,
    middleware::AddData,
    post,
    web::{
        websocket::{Message, WebSocket},
        Data, Multipart, Json, Path
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
use tokio::sync::watch::Sender;
use tokio::time::sleep;
mod lib;
use shared::models;
//use models::websocket::WebSocketMessage;


#[handler]
async fn upload(
    mut multipart: Multipart,
    installing_tasks: Data<&Arc<RwLock<HashMap<String, Child>>>>,
    currently_installing_projects: Data<&Arc<AtomicBool>>,
    clients: Data<&Arc<RwLock<HashMap<String, Sender<String>>>>>,
) -> String {
    match lib::upload(
        multipart,
        installing_tasks,
        currently_installing_projects,
        clients,
    )
    .await
    {
        Ok(response) => response,
        Err(err) => {
            // Server error
            return serde_json::to_string(&models::http::ErrorResponse::new(&err.to_string()))
                .unwrap();
        }
    }
}

#[handler]
async fn projects() -> String {
    match lib::projects().await {
        Ok(response) => response,
        Err(err) => {
            // Server error
            return serde_json::to_string(&models::http::ErrorResponse::new(&err.to_string()))
                .unwrap();
        }
    }
}

#[handler]
async fn tests(Path((project_id, script_id)): Path<(String, String)>) -> String {
    match lib::tests(&project_id, &script_id).await {
        Ok(response) => response,
        Err(err) => {
            // Server error
            return serde_json::to_string(&models::http::ErrorResponse::new(&err.to_string()))
                .unwrap();
        }
    }
}

#[handler]
async fn stop_test(
    Path((project_id, script_id, test_id)): Path<(String, String, String)>,

) -> String {
    match shared::get_worker_ip(&project_id, &script_id, &test_id) {
        Some(ip) => {
            let mut client = reqwest::Client::new();
            let mut response = client
                .post(&format!("http://{}/stop_test/{}/{}/{}", ip, project_id, script_id, test_id))
                .send()
                .await
                .unwrap();
            return response.text().await.unwrap();
        }
        None => {
            return String::from("No worker ip found");
        }
    }
    
}

#[handler]
pub async fn ws(
    ws: WebSocket,
    clients: Data<&Arc<RwLock<HashMap<String, Sender<String>>>>>,
    information_thread_running: Data<&Arc<AtomicBool>>,
) -> impl IntoResponse {
    let clients = Arc::clone(&clients);
    let tokio_information_thread_clients = Arc::clone(&clients);
    let information_thread_running = Arc::clone(&information_thread_running);
    ws.on_upgrade(move |socket| async move {
        let (tx, mut rx) = tokio::sync::watch::channel(String::from("channel"));
        let (mut sink, mut stream) = socket.split();
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros()
            .to_string();
        let id_rx = id.clone();
        let id_tx = id.clone();
        {
            let mut clients_guard = clients.write();
            clients_guard.insert(id, tx);
        }
        //websocket sender
        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                if let Message::Text(rec) = msg {
                    println!("WEBSOCKET: Received message: [{}], [{}]", rec, id_rx);

                }
            }
            let mut clients_guard = clients.write();
            clients_guard.remove(&id_rx);
            println!("WEBSOCKET: SENDER DISCONNECTED: [{}]", id_rx);
        });
        //websocket receiver
        tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                let msg = String::from(&*rx.borrow());
                if sink.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
            println!("WEBSOCKET: RECEIVER DISCONNECTED: [{}]", id_tx);
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
                        let connected_clients_count = guard.len() as u32;
                        let running_tests_count = 1;
                        let websocket_message = models::websocket::WebSocketMessage{
                            event_type: "INFORMATION",
                            event: models::websocket::information::Event{
                                connected_clients_count,
                                running_tests_count,
                            },
                        };
                        for (id, tx) in guard.iter() {
                            match tx.send(serde_json::to_string(&websocket_message).unwrap()) {
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
            println!("INFORMATION THREAD: Already running!");
        }
    })
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    //create download directory
    std::fs::create_dir_all(shared::get_downloads_dir()).unwrap();
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
        //.at("/path/:name/:id", get(path))
        .at("/upload", post(upload.data(currently_installing_projects)))
        .at("/ws", get(ws.data(information_thread_running)))
        .at("/projects", get(projects))
        .at("/tests/:project_id/:script_id", get(tests))
        .at("/stop_test/:project_id/:script_id/:test_id", post(stop_test))
        .nest(
            "/download",
            StaticFilesEndpoint::new(shared::get_downloads_dir())
                .show_files_listing(),
        )
        .with(AddData::new(installing_tasks))
        .with(AddData::new(clients));
    Server::new(TcpListener::bind("127.0.0.1:3000"))
        .run(app)
        .await
}

// #[handler]
// async fn single_download(req: poem::web::StaticFileRequest) -> poem::error::Result<impl IntoResponse> {
//     println!("sup");
//      Ok(req.create_response("path/to/file", true)?)
// }
