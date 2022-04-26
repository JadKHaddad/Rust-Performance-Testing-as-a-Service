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
        Data, Json, Multipart,
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
mod models;
//use models::websocket::WebSocketMessage;

#[handler]
async fn start_test(mut req: Json<models::http::TestParameter>, running_tests: Data<&Arc<RwLock<HashMap<String, Child>>>>,) -> String {
    match lib::start_test(req, running_tests).await {
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

    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "poem=debug");
    }

    //tests
    let running_tests: Arc<RwLock<HashMap<String, Child>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let currently_running_tests = Arc::new(AtomicBool::new(false));

    tracing_subscriber::fmt::init();

    let app = Route::new()
        .at("/start_test", post(start_test.data(running_tests)))
        .with(AddData::new(currently_running_tests));
    Server::new(TcpListener::bind("127.0.0.1:5000"))
        .run(app)
        .await
}

// #[handler]
// async fn single_download(req: poem::web::StaticFileRequest) -> poem::error::Result<impl IntoResponse> {
//     println!("sup");
//      Ok(req.create_response("path/to/file", true)?)
// }
