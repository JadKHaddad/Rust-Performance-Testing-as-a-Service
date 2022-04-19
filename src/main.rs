use parking_lot::RwLock;
use poem::{
    get, handler,
    listener::TcpListener,
    middleware::AddData,
    web::{Data, Html, Multipart},
    EndpointExt, Route, Server,
};
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    str,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{fs::File, io::AsyncWriteExt, time::sleep};

const PROJECTS_DIR: &str = "projects";
const ENVIRONMENTS_DIR: &str = "environments";

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
    let mut message = String::from("Project uploaded successfully!");
    let mut project_dir = PathBuf::new();
    let mut env_dir = PathBuf::new();
    let mut exists = false;
    let mut check = true;
    while let Ok(Some(field)) = multipart.next_field().await {
        if exists && check {
            continue;
        }
        //println!("{:?}", field);
        let file_name = field.file_name().map(ToString::to_string).unwrap();
        let project_name = Path::new(&file_name).components().next().unwrap();
        project_dir = Path::new(PROJECTS_DIR).join(&project_name);
        env_dir = Path::new(ENVIRONMENTS_DIR).join(&project_name);
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
    let pip_location_windows = Path::new(&env_dir).join("Scripts").join("pip3");
    println!("{:?}", pip_location_windows);
    let pip_location_linux = Path::new(&env_dir).join("bin").join("pip3");
    let mut cmd = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(&[
                "/c",
                &format!(
                    "virtualenv {} && {} install -r {}",
                    env_dir.to_str().unwrap(),
                    pip_location_windows.to_str().unwrap(),
                    requirements_file.to_str().unwrap()
                ),
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to execute process")
    } else {
        Command::new("/usr/bin/sh")
            .args(&[
                "-c",
                &format!(
                    "virtualenv {} && {} install -r {}",
                    env_dir.to_str().unwrap(),
                    pip_location_linux.to_str().unwrap(),
                    requirements_file.to_str().unwrap()
                ),
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to execute process")
    };
    let mut installing_tasks_guard = installing_tasks.write();
    let project_name = project_dir.file_name().unwrap();
    installing_tasks_guard.insert(project_name.to_str().unwrap().to_owned(), cmd);
    println!("{:?}", installing_tasks_guard);
    // run the thread
    if !currently_installing_projects.load(Ordering::SeqCst) {
        let tokio_currently_installing_projects = currently_installing_projects.clone();
        let tokio_installing_tasks = Arc::clone(&installing_tasks);
        tokio::spawn(async move {
            loop {
                let mut to_be_deleted: Vec<String> = Vec::new();
                {
                    let mut tokio_tasks_guard = tokio_installing_tasks.write();
                    if tokio_tasks_guard.len() < 1 {
                        tokio_currently_installing_projects.store(false, Ordering::SeqCst);
                        println!("PROJECTS GARBAGE COLLECTOR: Terminating!");
                        break;
                    }
                    println!("PROJECTS GARBAGE COLLECTOR: Running!");
                    let mut to_be_removed: Vec<String> = Vec::new();
                    for (id, cmd) in tokio_tasks_guard.iter_mut() {
                        match cmd.try_wait().unwrap() {
                            Some(exit_status) => {
                                to_be_removed.push(id.to_owned());
                                // delete on fail
                                match exit_status.code() {
                                    Some(code) => {
                                        println!("PROJECTS GARBAGE COLLECTOR: Project [{}] terminated with code [{}]!", id, code);
                                        if code != 0 {
                                            let err =
                                                child_stream_to_vec(cmd.stderr.take().unwrap());
                                            let err = str::from_utf8(&err).unwrap();
                                            to_be_deleted.push(id.to_owned());
                                            println!("PROJECTS GARBAGE COLLECTOR: Project [{}] terminated with error:\n{:?}", id, err);
                                        }
                                    }
                                    None => {
                                        println!("PROJECTS GARBAGE COLLECTOR: Project [{}] terminated by signal!", id);
                                    }
                                }
                            }
                            None => (),
                        }
                    }
                    for id in to_be_removed.iter() {
                        tokio_tasks_guard.remove_entry(id);
                        println!("PROJECTS GARBAGE COLLECTOR: Project [{}] removed!", id);
                    }
                }
                for id in to_be_deleted.iter() {
                    tokio::fs::remove_dir_all(Path::new(PROJECTS_DIR).join(id))
                        .await
                        .unwrap();
                    tokio::fs::remove_dir_all(Path::new(ENVIRONMENTS_DIR).join(id))
                        .await
                        .unwrap();
                    println!("PROJECTS GARBAGE COLLECTOR: Project [{}] deleted!", id);
                }
                sleep(Duration::from_secs(3)).await;
            }
        });
        currently_installing_projects.store(true, Ordering::SeqCst);
    }
    return message;
}

fn child_stream_to_vec<R>(mut stream: R) -> Vec<u8>
where
    R: Read + Send + 'static,
{
    let mut vec = Vec::new();
    loop {
        let mut buf = [0];
        match stream.read(&mut buf) {
            Err(err) => {
                println!("{}] Error reading from stream: {}", line!(), err);
                break;
            }
            Ok(got) => {
                if got == 0 {
                    break;
                } else if got == 1 {
                    vec.push(buf[0])
                } else {
                    println!("{}] Unexpected number of bytes: {}", line!(), got);
                    break;
                }
            }
        }
    }
    vec
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    //create projects dir
    tokio::fs::create_dir_all(PROJECTS_DIR).await.unwrap();
    //create environments dir
    tokio::fs::create_dir_all(ENVIRONMENTS_DIR).await.unwrap();
    //installing tasks
    let installing_tasks: Arc<RwLock<HashMap<String, Child>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let currently_installing_projects = Arc::new(AtomicBool::new(false));

    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "poem=debug");
    }
    tracing_subscriber::fmt::init();

    let app = Route::new()
        .at("/", get(index).post(upload))
        .with(AddData::new(installing_tasks))
        .with(AddData::new(currently_installing_projects));
    Server::new(TcpListener::bind("127.0.0.1:3000"))
        .run(app)
        .await
}
