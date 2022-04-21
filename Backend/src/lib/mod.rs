use crate::models;
use parking_lot::RwLock;
use poem::web::{Data, Multipart};
use std::error::Error;
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

pub const PROJECTS_DIR: &str = "projects";
pub const ENVIRONMENTS_DIR: &str = "environments";

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

pub async fn upload(
    mut multipart: Multipart,
    installing_tasks: Data<&Arc<RwLock<HashMap<String, Child>>>>,
    currently_installing_projects: Data<&Arc<AtomicBool>>,
    sender: Data<&tokio::sync::broadcast::Sender<String>>,
) -> Result<String, Box<dyn Error>> {
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
        let file_name = field
            .file_name()
            .map(ToString::to_string)
            .ok_or("Upload Error")?;
        let project_name = Path::new(&file_name)
            .components()
            .next()
            .ok_or("Upload Error")?;
        project_dir = Path::new(PROJECTS_DIR).join(&project_name);
        env_dir = Path::new(ENVIRONMENTS_DIR).join(&project_name);
        if project_dir.exists() && check {
            message = String::from("Project already exists");
            exists = true;
            check = false;
            continue;
        }
        let full_file_name = Path::new(PROJECTS_DIR).join(file_name);
        let full_file_name_prefix = full_file_name.parent().ok_or("Upload Error")?;
        tokio::fs::create_dir_all(full_file_name_prefix).await?;
        let mut file = File::create(full_file_name).await?;
        if let Ok(bytes) = field.bytes().await {
            file.write_all(&bytes).await?;
        }
        check = false;
    }
    if exists {
        return Ok(message);
    }
    // check if locust Folder exists and contains files
    let locust_dir = project_dir.join("locust");
    if !locust_dir.exists() {
        message = String::from("Locust folder empty or does not exist");
        //delete folder
        tokio::fs::remove_dir_all(project_dir).await?;
        return Ok(message);
    }
    // check if requirements.txt exists
    let requirements_file = project_dir.join("requirements.txt");
    if !requirements_file.exists() {
        message = String::from("No requirements.txt found");
        //delete folder
        tokio::fs::remove_dir_all(project_dir).await?;
        return Ok(message);
    }
    // check if requirements.txt contains locust
    let requirements_file_content = tokio::fs::read_to_string(&requirements_file).await?;
    if !requirements_file_content.contains("locust") {
        message = String::from("requirements.txt does not contain locust");
        //delete folder
        tokio::fs::remove_dir_all(project_dir).await?;
        return Ok(message);
    }

    //install
    let pip_location_windows = Path::new(&env_dir).join("Scripts").join("pip3");
    println!("{:?}", pip_location_windows);
    let pip_location_linux = Path::new(&env_dir).join("bin").join("pip3");
    let cmd = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(&[
                "/c",
                &format!(
                    "virtualenv {} && {} install -r {}",
                    env_dir.to_str().ok_or("Upload Error")?,
                    pip_location_windows.to_str().ok_or("Upload Error")?,
                    requirements_file.to_str().ok_or("Upload Error")?
                ),
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped())
            .spawn()?
    } else {
        Command::new("/usr/bin/sh")
            .args(&[
                "-c",
                &format!(
                    "virtualenv {} && {} install -r {}",
                    env_dir.to_str().ok_or("Upload Error")?,
                    pip_location_linux.to_str().ok_or("Upload Error")?,
                    requirements_file.to_str().ok_or("Upload Error")?
                ),
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped())
            .spawn()?
    };
    let mut installing_tasks_guard = installing_tasks.write();
    let project_name = project_dir.file_name().ok_or("Upload Error")?;

    installing_tasks_guard.insert(project_name.to_str().ok_or("Upload Error")?.to_owned(), cmd);
    println!("{:?}", installing_tasks_guard);
    // run the thread
    if !currently_installing_projects.load(Ordering::SeqCst) {
        let tx = sender.clone();
        let tokio_currently_installing_projects = currently_installing_projects.clone();
        let tokio_installing_tasks = Arc::clone(&installing_tasks);
        tokio::spawn(async move {
            loop {
                let mut to_be_deleted: Vec<String> = Vec::new();
                let mut installing_projects: Vec<models::projects::Project> = Vec::new();
                {
                    let mut tokio_tasks_guard = tokio_installing_tasks.write();
                    if tokio_tasks_guard.len() < 1 {
                        tokio_currently_installing_projects.store(false, Ordering::SeqCst);
                        println!("PROJECTS GARBAGE COLLECTOR: Terminating!");
                        break;
                    }
                    println!("PROJECTS GARBAGE COLLECTOR: Running!");
                    let mut to_be_removed: Vec<String> = Vec::new();
                    //collect info if a user is connected
                    for (id, cmd) in tokio_tasks_guard.iter_mut() {
                        let mut project = models::projects::Project {
                            id: id.to_owned(),
                            status: 0,
                            error: None,
                        };
                        match cmd.try_wait() {
                            Ok(Some(exit_status)) => {
                                // process finished
                                to_be_removed.push(id.to_owned());
                                project.status = 1;
                                // delete on fail
                                match exit_status.code() {
                                    Some(code) => {
                                        println!("PROJECTS GARBAGE COLLECTOR: Project [{}] terminated with code [{}]!", id, code);
                                        if code != 0 {
                                            project.status = 2;
                                            if let Some(stderr) = cmd.stderr.take() {
                                                let err = child_stream_to_vec(stderr);
                                                if let Ok(error_string) = str::from_utf8(&err) {
                                                    to_be_deleted.push(id.to_owned());
                                                    project.error = Some(error_string.to_owned());
                                                    println!("PROJECTS GARBAGE COLLECTOR: Project [{}] terminated with error:\n{:?}", id, error_string);
                                                }
                                            }
                                        }
                                    }
                                    None => {
                                        println!("PROJECTS GARBAGE COLLECTOR: Project [{}] terminated by signal!", id);
                                    }
                                }
                            }
                            Ok(None) => (), // process is running
                            Err(e) => {
                                println!("ERROR: PROJECTS GARBAGE COLLECTOR: Project [{}]: could not wait on child process error: {:?}", id, e);
                            }
                        }
                        installing_projects.push(project);
                    }
                    //remove finished
                    for id in to_be_removed.iter() {
                        tokio_tasks_guard.remove_entry(id);
                        println!("PROJECTS GARBAGE COLLECTOR: Project [{}] removed!", id);
                    }
                }
                // send info
                let websocket_message = models::WebSocketMessage{
                    event_type: "PROJECTS",
                    event: models::projects::Event{
                        istalling_projects: installing_projects,
                    },
                };
                match tx.send(serde_json::to_string(&websocket_message).unwrap()) {
                    Ok(_) => (),
                    Err(_) => (),
                }
                for id in to_be_deleted.iter() {
                    match tokio::fs::remove_dir_all(Path::new(PROJECTS_DIR).join(id)).await {
                        Ok(_) => (),
                        Err(e) => {
                            println!("ERROR: PROJECTS GARBAGE COLLECTOR: Project [{}]: folder could not be deleted!\n{:?}", id, e);
                        }
                    };
                    match tokio::fs::remove_dir_all(Path::new(ENVIRONMENTS_DIR).join(id)).await {
                        Ok(_) => (),
                        Err(e) => {
                            println!("ERROR: PROJECTS GARBAGE COLLECTOR: Project [{}]: environment could not be deleted!\n{:?}", id, e);
                        }
                    };
                    println!("PROJECTS GARBAGE COLLECTOR: Project [{}] deleted!", id);
                }
                sleep(Duration::from_secs(3)).await;
            }
        });
        currently_installing_projects.store(true, Ordering::SeqCst);
    } else {
        println!("PROJECTS GARBAGE COLLECTOR: Already running!");
    }
    Ok(message)
}

pub async fn projects() -> Result<String, Box<dyn Error>> {
    Ok("PROJECTS".to_owned())
}
