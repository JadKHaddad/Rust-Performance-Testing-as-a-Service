extern crate redis;
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use poem::{
    get, handler,
    listener::TcpListener,
    middleware::AddData,
    post,
    web::{
        websocket::{Message, WebSocket},
        Data, Json, Multipart, Path,
    },
    EndpointExt, IntoResponse, Route, Server,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::Ordering, Arc, Mutex},
    time::Duration,
};
mod lib;
use shared::models;
use std::process::Child;
use std::sync::atomic::AtomicU32;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::{sync::broadcast::Sender, time::sleep};

#[handler]
async fn health() -> String {
    format!("OK")
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
async fn upload(
    mut multipart: Multipart,
    installing_tasks: Data<&Arc<RwLock<HashMap<String, Child>>>>,
    currently_installing_projects: Data<&Arc<Mutex<bool>>>,
    main_sender: Data<&tokio::sync::broadcast::Sender<String>>,
) -> String {
    match lib::upload(
        multipart,
        installing_tasks,
        currently_installing_projects,
        main_sender,
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
async fn start_test(
    Path((project_id, script_id)): Path<(String, String)>,
    mut req: Json<models::http::TestInfo>,
    running_tests: Data<&Arc<RwLock<HashMap<String, lib::task::Task>>>>,
    currently_running_tests: Data<&Arc<Mutex<bool>>>,
    wanted_scripts: Data<&Arc<RwLock<HashSet<String>>>>,
) -> String {
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros()
        .to_string();
    let task_id = shared::encode_test_id(&project_id, &script_id, &id);
    match lib::start_test(
        &project_id,
        &script_id,
        req,
        running_tests,
        currently_running_tests,
        wanted_scripts,
        id,
        task_id.clone(),
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
async fn stop_test(
    Path((project_id, script_id, test_id)): Path<(String, String, String)>,
    running_tests: Data<&Arc<RwLock<HashMap<String, lib::task::Task>>>>,
) -> String {
    let task_id = shared::encode_test_id(&project_id, &script_id, &test_id);
    match lib::stop_test(&task_id, &running_tests /*red_client*/).await {
        Ok(response) => response,
        Err(err) => {
            // Server error
            return serde_json::to_string(&models::http::ErrorResponse::new(&err.to_string()))
                .unwrap();
        }
    }
}

#[handler]
async fn delete_test(
    Path((project_id, script_id, test_id)): Path<(String, String, String)>,
    running_tests: Data<&Arc<RwLock<HashMap<String, lib::task::Task>>>>,
) -> String {
    match lib::delete_test(
        &project_id,
        &script_id,
        &test_id,
        running_tests, /*red_client*/
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
async fn stop_script(
    Path((project_id, script_id)): Path<(String, String)>,
    running_tests: Data<&Arc<RwLock<HashMap<String, lib::task::Task>>>>,
) -> String {
    let script_id = shared::encode_script_id(&project_id, &script_id);
    match lib::stop_prefix(&script_id, running_tests).await {
        Ok(response) => response,
        Err(err) => {
            // Server error
            return serde_json::to_string(&models::http::ErrorResponse::new(&err.to_string()))
                .unwrap();
        }
    }
}

#[handler]
async fn stop_project(
    Path(project_id): Path<String>,
    running_tests: Data<&Arc<RwLock<HashMap<String, lib::task::Task>>>>,
) -> String {
    match lib::stop_prefix(&project_id, running_tests).await {
        Ok(response) => response,
        Err(err) => {
            // Server error
            return serde_json::to_string(&models::http::ErrorResponse::new(&err.to_string()))
                .unwrap();
        }
    }
}

#[handler]
async fn ws(
    ws: WebSocket,
    main_sender: Data<&tokio::sync::broadcast::Sender<String>>,
    connected_clients: Data<&Arc<AtomicU32>>,
    information_thread_running: Data<&Arc<Mutex<bool>>>,
    installing_tasks: Data<&Arc<RwLock<HashMap<String, Child>>>>,
) -> impl IntoResponse {
    let mut receiver = main_sender.subscribe();
    let tokio_main_sender = main_sender.clone();
    let tokio_connected_clients = connected_clients.clone();
    let ws_stream_connected_clients = connected_clients.clone();
    let ws_upgrade_connected_clients = connected_clients.clone();
    let information_thread_running = Arc::clone(&information_thread_running);
    let installing_tasks = installing_tasks.clone();
    ws.on_upgrade(move |socket| async move {
        let (mut sink, mut stream) = socket.split();
        ws_upgrade_connected_clients.fetch_add(1, Ordering::SeqCst);
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros()
            .to_string();

        println!(
            "[{}] WEBSOCKET: CONNECTED: [{}] | COUNT: [{}]",
            shared::get_date_and_time(),
            id,
            ws_upgrade_connected_clients.load(Ordering::SeqCst)
        );
        let id_tx = id.clone();
        //websocket sender
        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                if let Message::Text(rec_msg) = msg {
                    println!(
                        "[{}] WEBSOCKET: Received message: [{}], [{}]",
                        shared::get_date_and_time(),
                        rec_msg,
                        id
                    );
                }
            }
            ws_stream_connected_clients.fetch_sub(1, Ordering::SeqCst);
            println!(
                "[{}] WEBSOCKET: STREAM DISCONNECTED: [{}] | COUNT: [{}]",
                shared::get_date_and_time(),
                id,
                ws_stream_connected_clients.load(Ordering::SeqCst)
            );
        });
        //websocket listener
        tokio::spawn(async move {
            while let Ok(msg) = receiver.recv().await {
                if sink.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
            println!(
                "[{}] WEBSOCKET: LISTENER DROPPED: [{}]",
                shared::get_date_and_time(),
                id_tx
            );
        });

        //run information thread
        let mut information_thread_running_mutex = information_thread_running.lock().unwrap();
        if !*information_thread_running_mutex {
            *information_thread_running_mutex = true;
            println!(
                "[{}] INFORMATION THREAD: Running!",
                shared::get_date_and_time()
            );
            let tokio_information_thread_running = information_thread_running.clone();
            tokio::spawn(async move {
                loop {
                    let connected_clients_count = tokio_connected_clients.load(Ordering::SeqCst);
                    if connected_clients_count < 1 {
                        *tokio_information_thread_running.lock().unwrap() = false;
                        println!(
                            "[{}] INFORMATION THREAD: Terminating!",
                            shared::get_date_and_time()
                        );
                        break;
                    }
                    let istalling_projects;
                    {
                        let installing_tasks_guard = installing_tasks.read();
                        istalling_projects = installing_tasks_guard
                            .iter()
                            .map(|(k, _)| k.to_owned())
                            .collect::<Vec<_>>();
                    }
                    let mut running_tests_count: u32 = 0; //TODO

                    let websocket_message = models::websocket::WebSocketMessage {
                        event_type: shared::INFORMATION,
                        event: models::websocket::information::Event {
                            connected_clients_count,
                            running_tests_count,
                            istalling_projects,
                        },
                    };
                    if tokio_main_sender
                        .send(serde_json::to_string(&websocket_message).unwrap())
                        .is_err()
                    {
                        println!(
                            "[{}] INFORMATION THREAD: No clients are connected!",
                            shared::get_date_and_time()
                        );
                    }
                    sleep(Duration::from_secs(2)).await;
                }
            });
        } else {
            println!(
                "[{}] INFORMATION THREAD: Already running!",
                shared::get_date_and_time()
            );
        }
    })
}

#[handler]
async fn subscribe(
    other_ws: WebSocket,
    Path((project_id, script_id)): Path<(String, String)>,
    subscriptions: Data<&Arc<RwLock<HashMap<String, (u32, Sender<String>)>>>>,
) -> impl IntoResponse {
    let tokio_subscriptions = subscriptions.clone();
    let subscriptions = subscriptions.clone();
    other_ws.on_upgrade(move |socket| async move {
        //let mut red_connection = red_client.get_connection().unwrap();
        let (mut sink, mut stream) = socket.split();
        let script_id = if project_id == shared::CONTROL_SUB_STRING
            && script_id == shared::CONTROL_SUB_STRING
        {
            shared::CONTROL_SUB_STRING.to_string()
        } else {
            shared::encode_script_id(&project_id, &script_id)
        };

        let tokio_listener_script_id = script_id.clone();
        let script_id_debug = script_id.clone();
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros()
            .to_string();
        let id_tx = id.clone();

        let mut subscriptions_guard = subscriptions.write();
        if subscriptions_guard.contains_key(&script_id) {
            //update count
            let new_count = subscriptions_guard[&script_id].0 + 1;
            subscriptions_guard.get_mut(&script_id).unwrap().0 = new_count;
        } else {
            //create sender
            let sender = tokio::sync::broadcast::channel::<String>(32).0;

            subscriptions_guard.insert(script_id.clone(), (1, sender));
        }
        println!(
            "[{}] SUBSCRIBER: Script [{}]: COUNT: [{}]",
            shared::get_date_and_time(),
            script_id,
            subscriptions_guard[&script_id_debug].0
        );
        let sender = subscriptions_guard[&script_id_debug].1.clone();
        let mut receiver = subscriptions_guard[&script_id_debug].1.subscribe();
        //websocket sender
        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                if let Message::Text(rec_msg) = msg {
                    /*
                    println!(
                        "SUBSCRIBER: Script [{}]: Received message: [{}], [{}]",
                        script_id, rec_msg, id
                    );
                    */
                    if sender.send(rec_msg).is_err() {
                        break;
                    }
                }
            }
            println!(
                "[{}] SUBSCRIBER: Script [{}]: STREAM DISCONNECTED: [{}]",
                shared::get_date_and_time(),
                script_id,
                id
            );

            let mut subscriptions_guard = tokio_subscriptions.write();
            let new_count = subscriptions_guard[&script_id].0 - 1;
            println!(
                "[{}] SUBSCRIBER: Script [{}]: COUNT: [{}]",
                shared::get_date_and_time(),
                script_id,
                new_count
            );
            if new_count < 1 {
                subscriptions_guard.remove(&script_id);
            } else {
                subscriptions_guard.get_mut(&script_id).unwrap().0 = new_count;
            }
        });
        //websocket listener
        tokio::spawn(async move {
            while let Ok(msg) = receiver.recv().await {
                if sink.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
            println!(
                "[{}] WEBSOCKET: Script [{}]: LISTENER DROPPED: [{}]",
                shared::get_date_and_time(),
                tokio_listener_script_id,
                id_tx
            );
        });
    })
}

#[handler]
async fn project_scripts(Path(project_id): Path<String>) -> String {
    match lib::project_scripts(&project_id).await {
        Ok(response) => response,
        Err(err) => {
            // Server error
            return serde_json::to_string(&models::http::ErrorResponse::new(&err.to_string()))
                .unwrap();
        }
    }
}

#[handler]
async fn tests(
    Path((project_id, script_id)): Path<(String, String)>,
    running_tests: Data<&Arc<RwLock<HashMap<String, lib::task::Task>>>>,
) -> String {
    match lib::tests(&project_id, &script_id, running_tests).await {
        Ok(response) => response,
        Err(err) => {
            // Server error
            return serde_json::to_string(&models::http::ErrorResponse::new(&err.to_string()))
                .unwrap();
        }
    }
}

#[handler]
async fn control(running_tests: Data<&Arc<RwLock<HashMap<String, lib::task::Task>>>>) -> String {
    match lib::all_running_tests(running_tests).await {
        Ok(response) => response,
        Err(err) => {
            // Server error
            return serde_json::to_string(&models::http::ErrorResponse::new(&err.to_string()))
                .unwrap();
        }
    }
}

#[handler]
async fn delete_projects(
    projects_to_be_deleted: Json<models::http::projects::ProjectIds>,
    main_sender: Data<&tokio::sync::broadcast::Sender<String>>,
) -> String {
    match lib::delete_projects(projects_to_be_deleted, main_sender).await {
        Ok(response) => response,
        Err(err) => {
            // Server error
            return serde_json::to_string(&models::http::ErrorResponse::new(&err.to_string()))
                .unwrap();
        }
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), std::io::Error> {
    //main sender
    let main_sender = tokio::sync::broadcast::channel::<String>(512).0;

    let running_tests: Arc<RwLock<HashMap<String, lib::task::Task>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let currently_running_tests = Arc::new(Mutex::new(false));
    let wanted_scripts: Arc<RwLock<HashSet<String>>> = Arc::new(RwLock::new(HashSet::new()));
    let installing_tasks: Arc<RwLock<HashMap<String, Child>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let currently_installing_projects = Arc::new(Mutex::new(false));
    //clients
    let connected_clients = Arc::new(AtomicU32::new(0));
    let information_thread_running = Arc::new(Mutex::new(false));
    //subscriptions
    let subscriptions: Arc<RwLock<HashMap<String, (u32, Sender<String>)>>> =
        Arc::new(RwLock::new(HashMap::new()));
    tracing_subscriber::fmt::init();

    let app = Route::new()
        .at("/health", get(health))
        .at("/upload", post(upload.data(currently_installing_projects)))
        .at(
            "/ws",
            get(ws.data(information_thread_running).data(connected_clients)),
        )
        .at("/subscribe/:project_id/:script_id", get(subscribe))
        .at("/projects", get(projects))
        .at("/project/:project_id", get(project_scripts))
        .at("/tests/:project_id/:script_id", get(tests))
        .at("/control", get(control))
        .at("/start_test/:project_id/:script_id", post(start_test))
        .at(
            "/stop_test/:project_id/:script_id/:test_id",
            post(stop_test),
        )
        .at(
            "/delete_test/:project_id/:script_id/:test_id",
            post(delete_test),
        )
        .at("/delete_projects", post(delete_projects))
        .at("/stop_script/:project_id/:script_id", post(stop_script))
        .at("/stop_project/:project_id", post(stop_project))
        .with(AddData::new(running_tests))
        .with(AddData::new(currently_running_tests))
        .with(AddData::new(wanted_scripts))
        .with(AddData::new(installing_tasks))
        .with(AddData::new(subscriptions))
        .with(AddData::new(main_sender));

    Server::new(TcpListener::bind(format!("0.0.0.0:3000")))
        .run(app)
        .await
}

// #[handler]
// async fn single_download(req: poem::web::StaticFileRequest) -> poem::error::Result<impl IntoResponse> {
//     println!("sup");
//      Ok(req.create_response("path/to/file", true)?)
// }
