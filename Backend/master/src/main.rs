extern crate redis;
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
        Data, Multipart, Path,
    },
    EndpointExt, IntoResponse, Route, Server,
};
use redis::Commands;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    collections::HashMap,
    process::Child,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
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
    currently_installing_projects: Data<&Arc<AtomicBool>>,
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
pub async fn ws(
    ws: WebSocket,
    main_sender: Data<&tokio::sync::broadcast::Sender<String>>,
    connected_clients: Data<&Arc<AtomicU32>>,
    information_thread_running: Data<&Arc<AtomicBool>>,
    red_client: Data<&redis::Client>,
    installing_tasks: Data<&Arc<RwLock<HashMap<String, Child>>>>,
) -> impl IntoResponse {
    let mut receiver = main_sender.subscribe();
    let tokio_main_sender = main_sender.clone();
    let tokio_connected_clients = connected_clients.clone();
    let ws_stream_connected_clients = connected_clients.clone();
    let ws_upgrade_connected_clients = connected_clients.clone();
    let tokio_information_thread_running = information_thread_running.clone();
    let information_thread_running = Arc::clone(&information_thread_running);
    let red_client = red_client.clone();
    let installing_tasks = installing_tasks.clone();
    ws.on_upgrade(move |socket| async move {
        let (mut sink, mut stream) = socket.split();
        ws_upgrade_connected_clients.fetch_add(1, Ordering::SeqCst);
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros()
            .to_string();

        println!("WEBSOCKET: CONNECTED: [{}]", id);
        println!(
            "WEBSOCKET: COUNT: [{}]",
            ws_upgrade_connected_clients.load(Ordering::SeqCst)
        );
        let id_tx = id.clone();
        //websocket sender
        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                if let Message::Text(rec_msg) = msg {
                    println!("WEBSOCKET: Received message: [{}], [{}]", rec_msg, id);
                }
            }
            println!("WEBSOCKET: STREAM DISCONNECTED: [{}]", id);
            ws_stream_connected_clients.fetch_sub(1, Ordering::SeqCst);
            println!(
                "WEBSOCKET: COUNT: [{}]",
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
            println!("WEBSOCKET: LISTENER DROPPED: [{}]", id_tx);
        });

        //run information thread
        if !information_thread_running.load(Ordering::SeqCst) {
            println!("INFORMATION THREAD: Running!");
            let mut red_connection = red_client.get_connection().unwrap();
            tokio::spawn(async move {
                loop {
                    let connected_clients_count = tokio_connected_clients.load(Ordering::SeqCst);
                    if connected_clients_count < 1 {
                        tokio_information_thread_running.store(false, Ordering::SeqCst);
                        println!("INFORMATION THREAD: Terminating!");
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

                    let running_tests_count: u32 =
                        if let Ok(count) = red_connection.scard(shared::RUNNING_TESTS) {
                            count
                        } else {
                            0
                        };
                    let websocket_message = models::websocket::WebSocketMessage {
                        event_type: "INFORMATION",
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
                        println!("INFORMATION THREAD: No clients are connected!");
                    }

                    // {
                    //     let subscriptions_guard = subscriptions.read();
                    //     for (script_id, (_, sender)) in subscriptions_guard.iter() {
                    //         if let Ok(event) = red_connection.get(script_id) {
                    //             if sender.send(event).is_err() {
                    //                 println!("INFORMATION THREAD: No clients are connected!");
                    //             }
                    //         }
                    //     }
                    // }

                    sleep(Duration::from_secs(2)).await;
                }
            });
            information_thread_running.store(true, Ordering::SeqCst);
        } else {
            println!("INFORMATION THREAD: Already running!");
        }
    })
}

#[handler]
pub async fn subscribe(
    other_ws: WebSocket,
    Path((project_id, script_id)): Path<(String, String)>,
    subscriptions: Data<&Arc<RwLock<HashMap<String, (u32, Sender<String>)>>>>,
    red_client: Data<&redis::Client>,
) -> impl IntoResponse {
    let tokio_subscriptions = subscriptions.clone();
    let subscriptions = subscriptions.clone();
    let red_client = red_client.clone();
    other_ws.on_upgrade(move |socket| async move {
        let mut red_connection = red_client.get_connection().unwrap();
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
            let _: () = red_connection.sadd(shared::SUBS, &script_id).unwrap();
        }
        println!(
            "SUBSCRIBER: Script [{}]: COUNT: [{}]",
            script_id, subscriptions_guard[&script_id_debug].0
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
                "SUBSCRIBER: Script [{}]: STREAM DISCONNECTED: [{}]",
                script_id, id
            );

            let mut subscriptions_guard = tokio_subscriptions.write();
            let new_count = subscriptions_guard[&script_id].0 - 1;
            println!("SUBSCRIBER: Script [{}]: COUNT: [{}]", script_id, new_count);
            if new_count < 1 {
                subscriptions_guard.remove(&script_id);
                //update
                let _: () = red_connection.srem(shared::SUBS, &script_id).unwrap();
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
                "WEBSOCKET: Script [{}]: LISTENER DROPPED: [{}]",
                tokio_listener_script_id, id_tx
            );
        });
    })
}

#[handler]
pub async fn register_worker() -> impl IntoResponse {
    todo!()
}

#[handler]
pub async fn delete_worker() -> impl IntoResponse {
    todo!()
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let args: Vec<String> = std::env::args().collect();
    let mut port = "3000".to_owned();
    if let Some(port_) = args.get(1) {
        port = port_.to_owned();
    } else {
        println!("CONFIG: No port was given");
        if let Ok(port_) = std::env::var("PORT") {
            port = port_.to_owned();
        } else {
            println!("CONFIG: No port is set in environment");
        }
    }
    let mut redis_host = "127.0.0.1".to_owned();
    if let Some(r_host) = args.get(2) {
        redis_host = r_host.to_owned();
    } else {
        println!("CONFIG: No redis host was given");
        if let Ok(r_host) = std::env::var("REDIS_HOST") {
            redis_host = r_host.to_owned();
        } else {
            println!("CONFIG: No redis host is set in environment");
        }
    }
    let mut redis_port = "6379".to_owned();
    if let Some(r_port) = args.get(3) {
        redis_port = r_port.to_owned();
    } else {
        println!("CONFIG: No redis port was given");
        if let Ok(r_port) = std::env::var("REDIS_PORT") {
            redis_port = r_port.to_owned();
        } else {
            println!("CONFIG: No redis port is set in environment");
        }
    }

    println!(
        "[{}] MASTER: Starting on Port: [{}] with REDIS_HOST: [{}] | REDIS_PORT: [{}]\n",
        shared::get_date_and_time(), port, redis_host, redis_port
    );

    //create download directory
    std::fs::create_dir_all(shared::get_downloads_dir()).unwrap();
    //installing tasks
    let installing_tasks: Arc<RwLock<HashMap<String, Child>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let currently_installing_projects = Arc::new(AtomicBool::new(false));

    //clients
    let connected_clients = Arc::new(AtomicU32::new(0));
    let information_thread_running = Arc::new(AtomicBool::new(false));
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
    let mut red_connection = red_client.get_connection().unwrap();
    //reset subs on master start
    let _: () = red_connection.del(shared::SUBS).unwrap();

    //setup redis channel
    let pubsub_subscriptions = subscriptions.clone();
    thread::spawn(move || {
        let mut pubsub = red_connection.as_pubsub();
        pubsub.subscribe("main_channel").unwrap();
        loop {
            let msg = pubsub.get_message().unwrap();
            let payload: String = msg.get_payload().unwrap();
            //println!("channel '{}': {}", msg.get_channel_name(), payload);
            let redis_message: models::redis::RedisMessage =
                serde_json::from_str(&payload).unwrap();
            if redis_message.event_type == "UPDATE" || redis_message.event_type == "TEST_STOPPED" {
                let subscriptions_guard = pubsub_subscriptions.read();
                //println!("{:?}", subscriptions_guard);
                if let Some(sender) = &subscriptions_guard.get(&redis_message.id) {
                    if sender.1.send(redis_message.message).is_err() {
                        println!("REDIS CHANNEL THREAD: No clients are connected!");
                    };
                } else {
                    println!(
                        "REDIS CHANNEL THREAD: test [{}] was not found in running tests!",
                        redis_message.id
                    );
                }
            }
        }
    });

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
        .at(
            "/stop_test/:project_id/:script_id/:test_id",
            post(stop_test),
        )
        .at(
            "/delete_test/:project_id/:script_id/:test_id",
            post(delete_test),
        )
        .nest(
            "/download",
            StaticFilesEndpoint::new(shared::get_downloads_dir()).show_files_listing(),
        )
        .with(AddData::new(installing_tasks))
        .with(AddData::new(subscriptions))
        .with(AddData::new(main_sender))
        .with(AddData::new(red_client));
    Server::new(TcpListener::bind(format!("0.0.0.0:{}", port)))
        .run(app)
        .await
}

// #[handler]
// async fn single_download(req: poem::web::StaticFileRequest) -> poem::error::Result<impl IntoResponse> {
//     println!("sup");
//      Ok(req.create_response("path/to/file", true)?)
// }
