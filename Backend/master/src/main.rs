extern crate redis;
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use poem::{
    endpoint::StaticFilesEndpoint,
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
use redis::Commands;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    collections::HashMap,
    process::Child,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
use tokio::sync::broadcast::Sender;
use tokio::time::sleep;
mod lib;
use shared::models;

//use models::websocket::WebSocketMessage;

#[handler]
async fn health() -> String {
    "OK".to_string()
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
    red_client: Data<&redis::Client>,
) -> String {
    match lib::tests(&project_id, &script_id, red_client).await {
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
    subscriptions: Data<&Arc<RwLock<HashMap<String, (u32, Sender<String>)>>>>,
) -> String {
    match lib::stop_test(project_id, script_id, test_id, subscriptions).await {
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
    subscriptions: Data<&Arc<RwLock<HashMap<String, (u32, Sender<String>)>>>>,
    red_client: Data<&redis::Client>,
) -> String {
    match lib::delete_test(project_id, script_id, test_id, subscriptions, red_client).await {
        Ok(response) => response,
        Err(err) => {
            // Server error
            return serde_json::to_string(&models::http::ErrorResponse::new(&err.to_string()))
                .unwrap();
        }
    }
}

#[handler]
async fn download_test(
    req: poem::web::StaticFileRequest,
    Path((project_id, script_id, test_id)): Path<(String, String, String)>,
) -> poem::error::Result<impl IntoResponse> {
    let test_dir = shared::get_a_test_results_dir(&project_id, &script_id, &test_id);
    let zip_file = shared::get_zip_file(&project_id, &script_id, &test_id);
    let zip_file_str = zip_file.to_str().unwrap();
    let plot_file = shared::get_plot_file(&project_id, &script_id, &test_id);
    let plot_file_str = plot_file.to_str().unwrap();
    // if !zip_file.exists() {
    //     shared::zip::zip_folder(&test_dir.to_str().unwrap(), &zip_file_str).unwrap();
    // }
    //TODO!: stop the test before downloading
    shared::plot::plot(&project_id, &script_id, &test_id, &plot_file_str).unwrap();
    shared::zip::zip_folder(&test_dir.to_str().unwrap(), &zip_file_str).unwrap();
    Ok(req.create_response(&zip_file_str, true)?)
}

#[handler]
async fn ws(
    ws: WebSocket,
    main_sender: Data<&tokio::sync::broadcast::Sender<String>>,
    connected_clients: Data<&Arc<AtomicU32>>,
    information_thread_running: Data<&Arc<Mutex<bool>>>,
    //red_client: Data<&redis::Client>,
    red_manager: Data<&shared::Manager>,
    installing_tasks: Data<&Arc<RwLock<HashMap<String, Child>>>>,
) -> impl IntoResponse {
    let mut receiver = main_sender.subscribe();
    let tokio_main_sender = main_sender.clone();
    let tokio_connected_clients = connected_clients.clone();
    let ws_stream_connected_clients = connected_clients.clone();
    let ws_upgrade_connected_clients = connected_clients.clone();
    let information_thread_running = Arc::clone(&information_thread_running);
    let mut red_manager = red_manager.clone();
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
                    let mut running_tests_count: u32 = 0;

                    if let Ok(count) = red_manager.scard(shared::RUNNING_TESTS) {
                        running_tests_count = count;
                    }
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
    red_client: Data<&redis::Client>,
) -> impl IntoResponse {
    let tokio_subscriptions = subscriptions.clone();
    let subscriptions = subscriptions.clone();
    let red_client = red_client.clone();
    other_ws.on_upgrade(move |socket| async move {
        //let mut red_connection = red_client.get_connection().unwrap();
        let (mut sink, mut stream) = socket.split();
        let script_id = shared::encode_script_id(&project_id, &script_id);
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
            //save in redis
            if let Ok(mut connection) = red_client.get_connection() {
                let _: () = connection
                    .sadd(shared::SUBS, &script_id)
                    .unwrap_or_default();
            }
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
                //update
                if let Ok(mut connection) = red_client.get_connection() {
                    let _: () = connection
                        .srem(shared::SUBS, &script_id)
                        .unwrap_or_default();
                }
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
async fn stop_script(
    Path((project_id, script_id)): Path<(String, String)>,
    red_client: Data<&redis::Client>,
) -> String {
    match lib::stop_script(&project_id, &script_id, red_client).await {
        Ok(response) => response,
        Err(err) => {
            // Server error
            return serde_json::to_string(&models::http::ErrorResponse::new(&err.to_string()))
                .unwrap();
        }
    }
}

#[handler]
async fn delete_worker() -> String {
    //remove from redis
    "Ok".to_string()
}

#[handler]
async fn delete_projects(
    projects_to_be_deleted: Json<models::http::projects::ProjectIds>,
    red_client: Data<&redis::Client>,
    main_sender: Data<&tokio::sync::broadcast::Sender<String>>,
) -> String {
    match lib::delete_projects(projects_to_be_deleted, red_client, main_sender).await {
        Ok(response) => response,
        Err(err) => {
            // Server error
            return serde_json::to_string(&models::http::ErrorResponse::new(&err.to_string()))
                .unwrap();
        }
    }
}

#[handler]
async fn check_script(
    Path((project_id, script_id)): Path<(String, String)>,
) -> String {
    match lib::check_script(&project_id, &script_id) {
        Ok(response) => response,
        Err(err) => {
            // Server error
            return serde_json::to_string(&models::http::ErrorResponse::new(&err.to_string()))
                .unwrap();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let args: Vec<String> = std::env::args().collect();
    let mut port = "3000".to_owned();
    if let Some(port_) = args.get(1) {
        port = port_.to_owned();
    } else {
        println!(
            "[{}] CONFIG: No port was given",
            shared::get_date_and_time()
        );
        if let Ok(port_) = std::env::var("PORT") {
            port = port_.to_owned();
        } else {
            println!(
                "[{}] CONFIG: No port is set in environment",
                shared::get_date_and_time()
            );
        }
    }
    let mut redis_host = "127.0.0.1".to_owned();
    if let Some(r_host) = args.get(2) {
        redis_host = r_host.to_owned();
    } else {
        println!(
            "[{}] CONFIG: No redis host was given",
            shared::get_date_and_time()
        );
        if let Ok(r_host) = std::env::var("REDIS_HOST") {
            redis_host = r_host.to_owned();
        } else {
            println!(
                "[{}] CONFIG: No redis host is set in environment",
                shared::get_date_and_time()
            );
        }
    }
    let mut redis_port = "6379".to_owned();
    if let Some(r_port) = args.get(3) {
        redis_port = r_port.to_owned();
    } else {
        println!(
            "[{}] CONFIG: No redis port was given",
            shared::get_date_and_time()
        );
        if let Ok(r_port) = std::env::var("REDIS_PORT") {
            redis_port = r_port.to_owned();
        } else {
            println!(
                "[{}] CONFIG: No redis port is set in environment",
                shared::get_date_and_time()
            );
        }
    }

    println!(
        "[{}] MASTER: Starting on Port: [{}] with REDIS_HOST: [{}] | REDIS_PORT: [{}]\n",
        shared::get_date_and_time(),
        port,
        redis_host,
        redis_port
    );

    //create download directory
    //std::fs::create_dir_all(shared::get_downloads_dir()).unwrap();
    //workers
    //let workers: Arc<RwLock<HashSet<String>>> = Arc::new(RwLock::new(HashSet::new()));

    //remove temp folder on startup
    std::fs::remove_dir_all(shared::get_temp_dir()).unwrap_or_default();

    //installing tasks
    let installing_tasks: Arc<RwLock<HashMap<String, Child>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let currently_installing_projects = Arc::new(Mutex::new(false));

    //clients
    let connected_clients = Arc::new(AtomicU32::new(0));
    let information_thread_running = Arc::new(Mutex::new(false));
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "poem=debug");
    }

    //subscriptions
    let subscriptions: Arc<RwLock<HashMap<String, (u32, Sender<String>)>>> =
        Arc::new(RwLock::new(HashMap::new()));

    //main sender
    let main_sender = tokio::sync::broadcast::channel::<String>(512).0;

    //redis client
    let red_client =
        redis::Client::open(format!("redis://{}:{}/", redis_host, redis_port)).unwrap();
    //redis manager
    let manager = shared::Manager::new(red_client.clone()).await;
    //reset subs on master start
    loop {
        if let Ok(mut connection) = red_client.get_connection() {
            if let Ok(()) = connection.del(shared::SUBS) {
                break;
            }
        }
        eprintln!(
            "[{}] MASTER: Could not connect to redis. Trying again in 3 seconds.",
            shared::get_date_and_time()
        );
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    //setup redis channel
    let pubsub_subscriptions = subscriptions.clone();
    let pubsub_client = red_client.clone();
    thread::spawn(move || {
        loop {
            let mut red_connection;
            loop {
                if let Ok(connection) = pubsub_client.get_connection() {
                    red_connection = connection;
                    println!(
                        "[{}] MASTER: PUBSUB THREAD: Connected!",
                        shared::get_date_and_time()
                    );
                    break;
                }
                eprintln!(
                    "[{}] MASTER: PUBSUB THREAD: Could not connect to redis. Trying again in 3 seconds.",
                    shared::get_date_and_time()
                );
                std::thread::sleep(std::time::Duration::from_secs(3));
            }

            let mut pubsub = red_connection.as_pubsub();
            if pubsub.subscribe("main_channel").is_err() {
                eprintln!(
                    "[{}] MASTER: PUBSUB THREAD: Disconnected!",
                    shared::get_date_and_time()
                );
                std::thread::sleep(std::time::Duration::from_secs(3));
                continue;
            }
            loop {
                if let Ok(msg) = pubsub.get_message() {
                    if let Ok(payload) = msg.get_payload::<String>() {
                        let redis_message: models::redis::RedisMessage =
                            serde_json::from_str(&payload).unwrap();
                        if redis_message.event_type == shared::UPDATE_TEST_INFO
                            || redis_message.event_type == shared::TEST_STOPPED
                            || redis_message.event_type == shared::TEST_STARTED
                        {
                            let subscriptions_guard = pubsub_subscriptions.read();
                            //println!("{:?}", subscriptions_guard);
                            if let Some(sender) = &subscriptions_guard.get(&redis_message.id) {
                                if sender.1.send(redis_message.message).is_err() {
                                    eprintln!(
                                        "[{}] REDIS CHANNEL THREAD: No clients are connected!",
                                        shared::get_date_and_time()
                                    );
                                };
                            } else {
                                eprintln!(
                                "[{}] REDIS CHANNEL THREAD: test [{}] was not found in running tests!",
                                shared::get_date_and_time(),
                                redis_message.id
                            );
                            }
                        }
                    }
                } else {
                    eprintln!(
                        "[{}] MASTER: PUBSUB THREAD: Could not get message!",
                        shared::get_date_and_time()
                    );
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    break;
                }
            }
        }
    });

    tracing_subscriber::fmt::init();

    //run recovery thread
    let recovery_subscriptions = subscriptions.clone();
    let recovery_red_client = red_client.clone();
    tokio::spawn(async move {
        loop {
            let mut red_connection;
            loop {
                if let Ok(connection) = recovery_red_client.get_connection() {
                    red_connection = connection;
                    println!(
                        "[{}] MASTER: RECOVERY THREAD: Connected!",
                        shared::get_date_and_time()
                    );
                    break;
                }
                eprintln!(
                    "[{}] MASTER: RECOVERY THREAD: Could not connect to redis. Trying again in 3 seconds.",
                    shared::get_date_and_time()
                );
                sleep(Duration::from_secs(3)).await;
            }
            loop {
                let mut success = true;
                sleep(Duration::from_secs(10)).await;
                let subscriptions_guard = recovery_subscriptions.read();
                for sub in subscriptions_guard.keys() {
                    if let Err(e) = red_connection.sadd::<_, _, ()>(shared::SUBS, &sub) {
                        eprintln!(
                            "[{}] MASTER: RECOVERY THREAD: Disconnected! {}",
                            shared::get_date_and_time(),
                            e
                        );
                        success = false;
                        break;
                    }
                }
                if success {
                    // println!(
                    //     "[{}] MASTER: RECOVERY THREAD: Update!",
                    //     shared::get_date_and_time()
                    // );
                } else {
                    break;
                }
            }
        }
    });
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
        .at(
            "/stop_test/:project_id/:script_id/:test_id",
            post(stop_test),
        )
        .at(
            "/delete_test/:project_id/:script_id/:test_id",
            post(delete_test),
        )
        .at("/stop_script/:project_id/:script_id", post(stop_script))
        .at("/check_script/:project_id/:script_id", post(check_script))
        .at("/delete_projects", post(delete_projects))
        .at(
            "/download_test/:project_id/:script_id/:test_id",
            get(download_test),
        )
        .nest(
            "/explore",
            StaticFilesEndpoint::new(shared::get_data_dir())
                .show_files_listing()
                .prefer_utf8(true),
        )
        .with(AddData::new(installing_tasks))
        .with(AddData::new(subscriptions))
        .with(AddData::new(main_sender))
        .with(AddData::new(red_client))
        .with(AddData::new(manager));

    Server::new(TcpListener::bind(format!("0.0.0.0:{}", port)))
        .run(app)
        .await
}
