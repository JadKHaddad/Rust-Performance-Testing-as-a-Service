extern crate redis;
use parking_lot::RwLock;
use poem::{
    get, handler,
    listener::TcpListener,
    middleware::AddData,
    post,
    web::{Data, Json, Path},
    EndpointExt, Route, Server,
};
use redis::Commands;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;
mod lib;
use shared::models;
//use models::websocket::WebSocketMessage;

#[handler]
async fn health() -> String {
    "OK".to_string()
}

#[handler]
async fn start_test(
    Path((project_id, script_id)): Path<(String, String)>,
    mut req: Json<models::http::TestInfo>,
    running_tests: Data<&Arc<RwLock<HashMap<String, lib::task::Task>>>>,
    currently_running_tests: Data<&Arc<Mutex<bool>>>,
    red_client: Data<&redis::Client>,
    red_manager: Data<&shared::Manager>,
    ip: Data<&String>,
) -> String {
    match lib::start_test(
        &project_id,
        &script_id,
        req,
        running_tests,
        currently_running_tests,
        red_client,
        red_manager,
        ip,
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
    /*red_client: Data<&redis::Client>,*/
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
    /*red_client: Data<&redis::Client>,*/
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

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let args: Vec<String> = std::env::args().collect();

    let mut port = "5000".to_owned();
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
    //worker name is used to let the master communicate with the worker. Kubernetes Service name for example: worker_1
    //inside the cluster, the worker will be available under the name: worker_1
    let mut worker_name = format!("127.0.0.1:{}", port);
    if let Some(name) = args.get(2) {
        worker_name = name.to_owned();
    } else {
        println!(
            "[{}] CONFIG: No worker name was given",
            shared::get_date_and_time()
        );
        if let Ok(name) = std::env::var("WORKER_NAME") {
            worker_name = name.to_owned();
        } else {
            println!(
                "[{}] CONFIG: No worker name is set in environment",
                shared::get_date_and_time()
            );
        }
    }
    let mut master_ip = "127.0.0.1:3000".to_owned();
    if let Some(master_ip_) = args.get(3) {
        master_ip = master_ip_.to_owned()
    } else {
        println!(
            "[{}] CONFIG: No master ip was given",
            shared::get_date_and_time()
        );
        if let Ok(master_ip_) = std::env::var("MASTER_IP") {
            master_ip = master_ip_.to_owned()
        } else {
            println!(
                "[{}] CONFIG: No master ip is set in environment",
                shared::get_date_and_time()
            );
        }
    }
    let mut redis_host = "127.0.0.1".to_owned();
    if let Some(r_host) = args.get(4) {
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
    if let Some(r_port) = args.get(5) {
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
        "[{}] WORKER: Starting on Port: [{}] with WORKER_NAME: [{}] | MASTER_IP: [{}] | REDIS_HOST: [{}] | REDIS_PORT: [{}]\n",
        shared::get_date_and_time(), port, worker_name, master_ip, redis_host, redis_port
    );

    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "poem=debug");
    }

    //tests
    //let running_tests: Arc<RwLock<HashMap<String, Child>>> = Arc::new(RwLock::new(HashMap::new()));
    let running_tests: Arc<RwLock<HashMap<String, lib::task::Task>>> = Arc::new(RwLock::new(HashMap::new()));


    let currently_running_tests = Arc::new(Mutex::new(false));

    //redis client
    let red_client =
        redis::Client::open(format!("redis://{}:{}/", redis_host, redis_port)).unwrap();
    //redis manager
    let manager = shared::Manager::new(red_client.clone()).await;

    lib::register(&red_client, &worker_name);

    //remove running tests that belong to this worker
    lib::remove_all_running_tests(&red_client, &worker_name)
        .await
        .unwrap();

    tracing_subscriber::fmt::init();

    //run recovery thread
    let recovery_running_tests = running_tests.clone();
    let recovery_red_client = red_client.clone();
    let recovery_worker_name = worker_name.clone();
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
                "[{}] WORKER: RECOVERY THREAD: Could not connect to redis. Trying again in 3 seconds.",
                shared::get_date_and_time()
            );
                sleep(Duration::from_secs(3)).await;
            }
            loop {
                sleep(Duration::from_secs(10)).await;
                if let Err(e) = red_connection
                    .sadd::<_, _, ()>(shared::REGISTERED_WORKERS, &recovery_worker_name)
                {
                    eprintln!(
                        "[{}] WORKER: RECOVERY THREAD: Disconnected! {}",
                        shared::get_date_and_time(),
                        e
                    );
                    break;
                }
                let running_tests_guard = recovery_running_tests.read();
                for test in running_tests_guard.keys() {
                    if let Err(e) = red_connection.sadd::<_, _, ()>(shared::RUNNING_TESTS, &test) {
                        eprintln!(
                            "[{}] WORKER: RECOVERY THREAD: Disconnected! {}",
                            shared::get_date_and_time(),
                            e
                        );
                        break;
                    }
                }
                // println!(
                //     "[{}] WORKER: RECOVERY THREAD: Update!",
                //     shared::get_date_and_time()
                // );
            }
        }
    });
    let app = Route::new()
        .at("/health", get(health))
        .at("/start_test/:project_id/:script_id", post(start_test))
        .at(
            "/stop_test/:project_id/:script_id/:test_id",
            post(stop_test),
        )
        .at(
            "/delete_test/:project_id/:script_id/:test_id",
            post(delete_test),
        )
        .at("/stop_script/:project_id/:script_id", post(stop_script))
        .at("/stop_project/:project_id", post(stop_project))
        .with(AddData::new(worker_name))
        .with(AddData::new(running_tests))
        .with(AddData::new(currently_running_tests))
        .with(AddData::new(red_client))
        .with(AddData::new(manager));

    Server::new(TcpListener::bind(format!("0.0.0.0:{}", port)))
        .run(app)
        .await
}

// #[handler]
// async fn single_download(req: poem::web::StaticFileRequest) -> poem::error::Result<impl IntoResponse> {
//     println!("sup");
//      Ok(req.create_response("path/to/file", true)?)
// }
