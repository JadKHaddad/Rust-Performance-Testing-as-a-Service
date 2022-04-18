use poem::{
    get, handler,
    listener::TcpListener,
    web::{Html, Multipart},
    Route, Server,
};
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

const PROJECTS_DIR: &str = "projects";

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
async fn upload(mut multipart: Multipart) -> String {
    let mut message = String::from("Project uploaded successfully!");
    let mut project_dir = PathBuf::new();
    let mut exists = false;
    let mut check = true;
    while let Ok(Some(field)) = multipart.next_field().await {
        if exists && check {
            continue;
        }
        //println!("{:?}", field);
        let file_name = field.file_name().map(ToString::to_string).unwrap();
        let project_name = Path::new(&file_name).components().next().unwrap();
        project_dir = Path::new(PROJECTS_DIR).join(project_name);
        if project_dir.exists() && check {
            message = String::from("Project already exists");
            exists = true;
            check = false;
            continue;
        }
        let full_file_name = Path::new(PROJECTS_DIR).join(file_name);
        let full_file_name_prefix = full_file_name.parent().unwrap();
        tokio::fs::create_dir_all(full_file_name_prefix)
            .await
            .unwrap();
        let mut file = File::create(full_file_name).await.unwrap();
        if let Ok(bytes) = field.bytes().await {
            file.write_all(&bytes).await.unwrap();
        }
        check = false;
    }
    if exists {
        return message;
    }
    // check if locust Folder exists and contains files
    let locust_dir = project_dir.join("locust");
    if !locust_dir.exists() {
        message = String::from("Locust folder empty or does not exist");
        //delete folder
        tokio::fs::remove_dir_all(project_dir).await.unwrap();
        return message;
    }
    // check if requirements.txt exists
    let requirements_file = project_dir.join("requirements.txt");
    if !requirements_file.exists() {
        message = String::from("No requirements.txt found");
        //delete folder
        tokio::fs::remove_dir_all(project_dir).await.unwrap();
        return message;
    }
    // check if requirements.txt contains locust
    let requirements_file_content = tokio::fs::read_to_string(&requirements_file).await.unwrap();
    if !requirements_file_content.contains("locust") {
        message = String::from("requirements.txt does not contain locust");
        //delete folder
        tokio::fs::remove_dir_all(project_dir).await.unwrap();
        return message;
    }

    //install
    return message;
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    //create projects dir
    tokio::fs::create_dir_all(PROJECTS_DIR).await.unwrap();

    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "poem=debug");
    }
    tracing_subscriber::fmt::init();

    let app = Route::new().at("/", get(index).post(upload));
    Server::new(TcpListener::bind("127.0.0.1:3000"))
        .run(app)
        .await
}
