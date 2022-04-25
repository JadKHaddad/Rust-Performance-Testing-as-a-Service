use crate::models;
use crate::{DATA_DIR, ENVIRONMENTS_DIR, PROJECTS_DIR, RESULTS_DIR, TEMP_DIR};
use parking_lot::RwLock;
use poem::web::{Data, Json, Multipart};
use std::error::Error;
use std::fs::canonicalize;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};
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
use tokio::sync::watch::Sender;
use tokio::time::sleep;

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

fn move_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(&src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            move_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    std::fs::remove_dir_all(src)?;
    Ok(())
}

fn get_temp_dir() -> PathBuf {
    Path::new(DATA_DIR).join(TEMP_DIR)
}

fn get_projects_dir() -> PathBuf {
    Path::new(DATA_DIR).join(PROJECTS_DIR)
}

fn get_environments_dir() -> PathBuf {
    Path::new(DATA_DIR).join(ENVIRONMENTS_DIR)
}

fn get_results_dir() -> PathBuf {
    Path::new(DATA_DIR).join(RESULTS_DIR)
}

fn get_a_project_dir(id: &str) -> PathBuf {
    get_projects_dir().join(id)
}

fn get_a_temp_dir(id: &str) -> PathBuf {
    get_temp_dir().join(id)
}

fn get_an_environment_dir(id: &str) -> PathBuf {
    get_environments_dir().join(id)
}

fn get_a_locust_dir(id: &str) -> PathBuf {
    get_a_project_dir(id).join("locust")
}

fn get_a_project_results_dir(id: &str) -> PathBuf {
    get_results_dir().join(id)
}

fn get_a_script_results_dir(project_id: &str, script_id: &str) -> PathBuf {
    get_a_project_results_dir(project_id).join(script_id)
}

fn get_a_test_results_dir(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    get_a_script_results_dir(project_id, script_id).join(test_id)
}

fn get_test_id(project_id: &str, script_id: &str, test_id: &str) -> String {
    format!("$[{}]$[{}]$[{}]$", project_id, script_id, test_id)
}

fn get_log_file_relative_path(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    Path::new("../..")
        .join(RESULTS_DIR)
        .join(project_id)
        .join(script_id)
        .join(test_id)
        .join("log.log")
}

fn get_csv_file_relative_path(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    Path::new("../..")
        .join(RESULTS_DIR)
        .join(project_id)
        .join(script_id)
        .join(test_id)
        .join("results.csv")
}

pub async fn upload(
    // must lock
    mut multipart: Multipart,
    installing_tasks: Data<&Arc<RwLock<HashMap<String, Child>>>>,
    currently_installing_projects: Data<&Arc<AtomicBool>>,
    clients: Data<&Arc<RwLock<HashMap<String, Sender<String>>>>>,
) -> Result<String, Box<dyn Error>> {
    let mut response = models::http::Response::<String> {
        success: true,
        message: "Uploading project",
        error: None,
        content: None,
    };
    let mut project_temp_dir = PathBuf::new();
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
        project_temp_dir = get_temp_dir().join(&project_name);
        let project_dir = get_projects_dir().join(&project_name);
        env_dir = get_environments_dir().join(&project_name);
        if (project_temp_dir.exists() && check) || project_dir.exists() && check {
            response.error = Some("Project already exists");
            response.success = false;
            exists = true;
            check = false;
            continue;
        }
        let full_file_name = get_temp_dir().join(file_name);
        let full_file_name_prefix = full_file_name.parent().ok_or("Upload Error")?;
        std::fs::create_dir_all(full_file_name_prefix)?;
        let mut file = std::fs::File::create(full_file_name)?;
        if let Ok(bytes) = field.bytes().await {
            file.write(&bytes)?;
        }
        check = false;
    }
    if exists {
        return Ok(serde_json::to_string(&response).unwrap());
    }
    // check if locust Folder exists and contains files
    let locust_dir = project_temp_dir.join("locust");
    if !locust_dir.exists() {
        response.error = Some("Locust folder empty or does not exist");
        response.success = false;
        //delete folder
        std::fs::remove_dir_all(project_temp_dir)?;
        return Ok(serde_json::to_string(&response).unwrap());
    }
    // check if requirements.txt exists
    let requirements_file = project_temp_dir.join("requirements.txt");
    if !requirements_file.exists() {
        response.error = Some("No requirements.txt found");
        response.success = false;
        //delete folder
        std::fs::remove_dir_all(project_temp_dir)?;
        return Ok(serde_json::to_string(&response).unwrap());
    }
    // check if requirements.txt contains locust
    let requirements_file_content = std::fs::read_to_string(&requirements_file)?;
    if !requirements_file_content.contains("locust") {
        response.error = Some("requirements.txt does not contain locust");
        response.success = false;
        //delete folder
        std::fs::remove_dir_all(project_temp_dir)?;
        return Ok(serde_json::to_string(&response).unwrap());
    }

    //install
    let cmd = if cfg!(target_os = "windows") {
        let pip_location_windows = Path::new(&env_dir).join("Scripts").join("pip3");
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
        let pip_location_linux = Path::new(&env_dir).join("bin").join("pip3");
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
    let project_name = project_temp_dir.file_name().ok_or("Upload Error")?;

    installing_tasks_guard.insert(project_name.to_str().ok_or("Upload Error")?.to_owned(), cmd);
    println!("{:?}", installing_tasks_guard);
    // run the thread
    if !currently_installing_projects.load(Ordering::SeqCst) {
        let clients = Arc::clone(&clients);
        let tokio_currently_installing_projects = currently_installing_projects.clone();
        let tokio_installing_tasks = Arc::clone(&installing_tasks);
        tokio::spawn(async move {
            loop {
                let mut to_be_deleted: Vec<String> = Vec::new();
                let mut installing_projects: Vec<models::websocket::projects::Project> = Vec::new();
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
                        let mut project = models::websocket::projects::Project {
                            id: id.to_owned(),
                            status: 0,
                            error: None,
                        };
                        match cmd.try_wait() {
                            Ok(Some(exit_status)) => {
                                // process finished
                                to_be_removed.push(id.to_owned());
                                project.status = 2;
                                // delete on fail
                                match exit_status.code() {
                                    Some(code) => {
                                        println!("PROJECTS GARBAGE COLLECTOR: Project [{}] terminated with code [{}]!", id, code);
                                        if code != 0 {
                                            if let Some(stderr) = cmd.stderr.take() {
                                                let err = child_stream_to_vec(stderr);
                                                if let Ok(error_string) = str::from_utf8(&err) {
                                                    to_be_deleted.push(id.to_owned());
                                                    project.error = Some(error_string.to_owned());
                                                    println!("PROJECTS GARBAGE COLLECTOR: Project [{}] terminated with error:\n{:?}", id, error_string);
                                                }
                                            }
                                        } else {
                                            project.status = 1; // process finished
                                                                // move to installed projects
                                            match move_dir_all(
                                                get_a_temp_dir(id),
                                                get_a_project_dir(id),
                                            ) {
                                                Ok(_) => {
                                                    println!("PROJECTS GARBAGE COLLECTOR: Project [{}] moved to installed projects!", id);
                                                }
                                                Err(e) => {
                                                    println!("PROJECTS GARBAGE COLLECTOR: Project [{}] failed to move to installed projects!\n{:?}", id, e);
                                                }
                                            }
                                        }
                                    }
                                    None => {
                                        println!("PROJECTS GARBAGE COLLECTOR: Project [{}] terminated by signal!", id);
                                    }
                                }
                            }
                            Ok(None) => {
                                project.status = 0; // process is running
                            }
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
                //delete not valid
                for id in to_be_deleted.iter() {
                    match std::fs::remove_dir_all(get_a_temp_dir(id)) {
                        Ok(_) => (),
                        Err(e) => {
                            println!("ERROR: PROJECTS GARBAGE COLLECTOR: Project [{}]: folder could not be deleted!\n{:?}", id, e);
                        }
                    };
                    match std::fs::remove_dir_all(get_an_environment_dir(id)) {
                        Ok(_) => (),
                        Err(e) => {
                            println!("ERROR: PROJECTS GARBAGE COLLECTOR: Project [{}]: environment could not be deleted!\n{:?}", id, e);
                        }
                    };
                    println!("PROJECTS GARBAGE COLLECTOR: Project [{}] deleted!", id);
                }
                // send info
                let websocket_message = models::websocket::WebSocketMessage {
                    event_type: "PROJECTS",
                    event: models::websocket::projects::Event {
                        istalling_projects: installing_projects,
                    },
                };
                for (id, tx) in clients.read().iter() {
                    match tx.send(serde_json::to_string(&websocket_message).unwrap()) {
                        Ok(_) => {}
                        Err(e) => {
                            println!(
                                "PROJECTS GARBAGE COLLECTOR: failed to send message [{}]:\n{:?}",
                                id, e
                            );
                        }
                    }
                }
                sleep(Duration::from_secs(3)).await;
            }
        });
        currently_installing_projects.store(true, Ordering::SeqCst);
    } else {
        println!("PROJECTS GARBAGE COLLECTOR: Already running!");
    }
    Ok(serde_json::to_string(&response).unwrap())
}

pub async fn projects() -> Result<String, Box<dyn Error>> {
    let mut response = models::http::Response {
        success: true,
        message: "Installed Projects",
        error: None,
        content: None,
    };
    let mut content = models::http::projects::Content {
        projects: Vec::new(),
    };
    let projects_dir = match std::fs::read_dir(get_projects_dir()) {
        Ok(dir) => dir,
        Err(_) => {
            response.content = Some(content);
            let response = serde_json::to_string(&response).unwrap();
            return Ok(response);
        }
    };
    for project_dir in projects_dir {
        let project_name = project_dir?
            .file_name()
            .to_str()
            .ok_or("Parse Error")?
            .to_owned();
        let locust_dir = match std::fs::read_dir(get_a_locust_dir(&project_name)) {
            Ok(dir) => dir,
            Err(_) => {
                continue;
            }
        };
        let mut scripts = Vec::new();
        for script_file in locust_dir {
            let script_name = script_file?
                .file_name()
                .to_str()
                .ok_or("Parse Error")?
                .to_owned();
            scripts.push(script_name);
        }
        content.projects.push(models::http::projects::Project {
            id: project_name,
            scripts: scripts,
        });
    }
    response.content = Some(content);
    let response = serde_json::to_string(&response).unwrap();
    Ok(response)
}

pub async fn start_test(
    mut req: Json<models::http::TestParameter>,
    running_tests: Data<&Arc<RwLock<HashMap<String, Child>>>>,
) -> Result<String, Box<dyn Error>> {
    println!("{:?}", req);
    let project_id = &req.project_id;
    let script_id = &req.script_id;
    //let workers = req.workers.unwrap_or(1);

    let locust_file = get_a_locust_dir(project_id).join(script_id);

    //checking here if the locust file exists and then we will check again before running if the script was in the meantime deleted
    if !locust_file.exists() {
        return Ok(String::from("Script not found!"));
    }

    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros()
        .to_string();

    //create test dir
    let test_dir = get_a_test_results_dir(project_id, script_id, &id);
    std::fs::create_dir_all(&test_dir)?;

    //define paths
    let env_dir = get_an_environment_dir(&project_id);
    let can_project_dir = canonicalize(get_a_project_dir(&project_id)).unwrap();
    let can_locust_file = canonicalize(&locust_file).unwrap();
    let log_file_relative_path = get_log_file_relative_path(project_id, script_id, &id);
    let csv_file_relative_path = get_csv_file_relative_path(project_id, script_id, &id);

    //create commands
    let users_command = if let Some(req_users) = &req.users {
        format!("--users {}", req_users)
    } else {
        String::new()
    };
    let spawn_rate_command = if let Some(req_spawn_rate) = &req.spawn_rate {
        format!("--spawn-rate {}", req_spawn_rate)
    } else {
        String::new()
    };
    let time_command = if let Some(req_time) = &req.time {
        format!("--run-time {}s", req_time)
    } else {
        String::new()
    };
    let host_command = if let Some(req_host) = &req.host {
        format!("--host {}", req_host)
    } else {
        String::new()
    };
    let log_command = format!(
        "--logfile {}",
        log_file_relative_path.to_str().ok_or("Run Error")?
    );
    let csv_command = format!(
        "--csv {}",
        csv_file_relative_path.to_str().ok_or("Run Error")?
    );
    //let workers_command = format!("--workers {}", workers);

    //lock before running
    let mut running_tests_guard = running_tests.write();
    //checking if the script was not deleted in the meantime after performing the lock
    if !locust_file.exists() {
        return Ok(String::from("Script was deleted!"));
    }
    //run
    let cmd = if cfg!(target_os = "windows") {
        let can_locust_location_windows =
            canonicalize(Path::new(&env_dir).join("Scripts").join("locust.exe")).unwrap();

        Command::new("powershell")
            .current_dir(can_project_dir)
            .args(&[
                "/c",
                &format!(
                    "{} -f {} --headless {} {} {} {} {} {}",
                    can_locust_location_windows.to_str().ok_or("Run Error")?,
                    can_locust_file.to_str().ok_or("Run Error")?,
                    users_command,
                    spawn_rate_command,
                    time_command,
                    host_command,
                    log_command,
                    csv_command,
                ),
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?
    } else {
        // let can_locust_location_linux =
        //     canonicalize(Path::new(&env_dir).join("bin").join("locust")).unwrap();
        Command::new("/usr/bin/sh")
            .current_dir(get_a_project_dir(&project_id))
            .args(&["-c"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?
    };

    let test_id = get_test_id(&project_id, &script_id, &id);
    //save test info as json
    req.id = Some(id.clone());
    let mut file = std::fs::File::create(&test_dir.join("info.json"))?;
    file.write(serde_json::to_string(&*req).unwrap().as_bytes())?;

    running_tests_guard.insert(test_id, cmd);

    //run the garbage collector
    Ok(String::from("allright"))
}
