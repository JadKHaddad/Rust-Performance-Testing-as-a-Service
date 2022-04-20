use parking_lot::RwLock;
use poem::{
    handler,
    listener::TcpListener,
    middleware::AddData,
    post,
    web::{Data, Html, Multipart},
    EndpointExt, Route, Server,
};
use std::{
    collections::HashMap,
    process::Child,
    str,
    sync::{atomic::AtomicBool, Arc},
};
mod lib;

#[handler]
async fn index() -> Html<&'static str> {
    Html(
        r###"
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <title>Poem / Upload Example</title>
        </head>
        <body>
            <form action="/" enctype="multipart/form-data" method="post">
                <input type="file" webkitdirectory="" mozdirectory="" name="upload" id="files">
                <button type="submit">Submit</button>
            </form>
        </body>
        </html>
        "###,
    )
}

#[handler]
async fn upload(
    mut multipart: Multipart,
    installing_tasks: Data<&Arc<RwLock<HashMap<String, Child>>>>,
    currently_installing_projects: Data<&Arc<AtomicBool>>,
) -> String {
    match lib::upload(multipart, installing_tasks, currently_installing_projects).await {
        Ok(message) => message,
        Err(err) => err.to_string(),
    }
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

    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "poem=debug");
    }
    tracing_subscriber::fmt::init();

    let app = Route::new()
        .at("/upload", post(upload))
        .with(AddData::new(installing_tasks))
        .with(AddData::new(currently_installing_projects));
    Server::new(TcpListener::bind("127.0.0.1:3000"))
        .run(app)
        .await
}
