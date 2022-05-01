extern crate redis;

use parking_lot::RwLock;
use poem::{
    handler,
    listener::TcpListener,
    middleware::AddData,
    post,
    web::{Data, Json, Path},
    EndpointExt, Route, Server,
};
use std::{
    collections::HashMap,
    process::Child,
    sync::{atomic::AtomicBool, Arc},
};
mod lib;
use shared::models;
//use models::websocket::WebSocketMessage;

#[handler]
async fn start_test(
    Path((project_id, script_id)): Path<(String, String)>,
    mut req: Json<models::http::TestInfo>,
    running_tests: Data<&Arc<RwLock<HashMap<String, Child>>>>,
    currently_running_tests: Data<&Arc<AtomicBool>>,
    red_client: Data<&redis::Client>,
    ip: Data<&String>,
) -> String {
    match lib::start_test(
        &project_id,
        &script_id,
        req,
        running_tests,
        currently_running_tests,
        red_client,
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
    running_tests: Data<&Arc<RwLock<HashMap<String, Child>>>>,
) -> String {
    match lib::stop_test(&project_id, &script_id, &test_id, running_tests).await {
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
    //IP
    let ip = String::from("127.0.0.1:5000");

    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "poem=debug");
    }

    //tests
    let running_tests: Arc<RwLock<HashMap<String, Child>>> = Arc::new(RwLock::new(HashMap::new()));
    let currently_running_tests = Arc::new(AtomicBool::new(false));

    //redis client
    let red_client = redis::Client::open(format!("redis://{}:{}/", "localhost", "6379")).unwrap();

    tracing_subscriber::fmt::init();

    let app = Route::new()
        .at("/start_test/:project_id/:script_id", post(start_test))
        .at(
            "/stop_test/:project_id/:script_id/:test_id",
            post(stop_test),
        )
        .with(AddData::new(ip))
        .with(AddData::new(running_tests))
        .with(AddData::new(currently_running_tests))
        .with(AddData::new(red_client));

    Server::new(TcpListener::bind("127.0.0.1:5000"))
        .run(app)
        .await
}

// #[handler]
// async fn single_download(req: poem::web::StaticFileRequest) -> poem::error::Result<impl IntoResponse> {
//     println!("sup");
//      Ok(req.create_response("path/to/file", true)?)
// }
