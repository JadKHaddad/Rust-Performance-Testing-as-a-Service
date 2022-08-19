use parking_lot::RwLock;
use poem::web::{Data, Multipart};
use redis::Commands;
use shared::models;
use std::error::Error;
use std::io::Write;
use std::{
    collections::{HashMap, HashSet},
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
use tokio::sync::broadcast::Sender;
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
                eprintln!("{}] Error reading from stream: {}", line!(), err);
                break;
            }
            Ok(got) => {
                if got == 0 {
                    break;
                } else if got == 1 {
                    vec.push(buf[0])
                } else {
                    eprintln!("{}] Unexpected number of bytes: {}", line!(), got);
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

pub async fn upload(
    // must lock
    mut multipart: Multipart,
    installing_tasks: Data<&Arc<RwLock<HashMap<String, Child>>>>,
    currently_installing_projects: Data<&Arc<AtomicBool>>,
    main_sender: Data<&tokio::sync::broadcast::Sender<String>>,
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
        let re = regex::Regex::new(r"\s+").unwrap();
        let file_name = re.replace_all(&file_name, "_").into_owned();
        let project_name = Path::new(&file_name)
            .components()
            .next()
            .ok_or("Upload Error")?;
        project_temp_dir = shared::get_temp_dir().join(&project_name);
        let project_dir = shared::get_projects_dir().join(&project_name);
        env_dir = shared::get_environments_dir().join(&project_name);
        if (project_temp_dir.exists() && check) || project_dir.exists() && check {
            response.error = Some("Project already exists");
            response.success = false;
            exists = true;
            check = false;
            continue;
        }
        let full_file_name = shared::get_temp_dir().join(file_name);
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
        Command::new("bash")
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
    // run the thread
    let main_sender = main_sender.clone();
    if !currently_installing_projects.load(Ordering::SeqCst) {
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
                                                shared::get_a_temp_dir(id),
                                                shared::get_a_project_dir(id),
                                            ) {
                                                Ok(_) => {
                                                    println!("PROJECTS GARBAGE COLLECTOR: Project [{}] moved to installed projects!", id);
                                                }
                                                Err(e) => {
                                                    eprintln!("ERROR: PROJECTS GARBAGE COLLECTOR: Project [{}] failed to move to installed projects!\n{:?}", id, e);
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
                                eprintln!("ERROR: PROJECTS GARBAGE COLLECTOR: Project [{}]: could not wait on child process error: {:?}", id, e);
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
                    match std::fs::remove_dir_all(shared::get_a_temp_dir(id)) {
                        Ok(_) => (),
                        Err(e) => {
                            eprintln!("ERROR: PROJECTS GARBAGE COLLECTOR: Project [{}]: folder could not be deleted!\n{:?}", id, e);
                        }
                    };
                    match std::fs::remove_dir_all(shared::get_an_environment_dir(id)) {
                        Ok(_) => (),
                        Err(e) => {
                            eprintln!("ERROR: PROJECTS GARBAGE COLLECTOR: Project [{}]: environment could not be deleted!\n{:?}", id, e);
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

                if main_sender
                    .send(serde_json::to_string(&websocket_message).unwrap())
                    .is_err()
                {
                    println!("PROJECTS GARBAGE COLLECTOR: No clients are connected!");
                }
                sleep(Duration::from_secs(3)).await;
            }
        });
        currently_installing_projects.store(true, Ordering::SeqCst); //TODO: maybe move up? before the thread?
    } else {
        println!("PROJECTS GARBAGE COLLECTOR: Already running!");
    }
    Ok(serde_json::to_string(&response).unwrap())
}

pub async fn projects() -> Result<String, Box<dyn Error>> {
    let mut response = shared::models::http::Response {
        success: true,
        message: "Installed Projects",
        error: None,
        content: None,
    };
    let mut content = models::http::projects::Content {
        projects: Vec::new(),
    };
    let projects_dir = match std::fs::read_dir(shared::get_projects_dir()) {
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
        let locust_dir = match std::fs::read_dir(shared::get_a_locust_dir(&project_name)) {
            Ok(dir) => dir,
            Err(_) => {
                continue;
            }
        };
        let mut scripts = Vec::new();
        for script_file in locust_dir {
            let script_file = script_file?;
            if script_file.metadata()?.is_dir() {
                continue;
            }
            let script_name = script_file
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

pub async fn project_scripts(project_id: &str) -> Result<String, Box<dyn Error>> {
    let mut response = shared::models::http::Response {
        success: true,
        message: "Project",
        error: None,
        content: None,
    };
    let mut content = models::http::scripts::Content {
        scripts: Vec::new(),
    };

    let locust_dir = std::fs::read_dir(shared::get_a_locust_dir(project_id))?;

    for script_file in locust_dir {
        let script_file = script_file?;
        if script_file.metadata()?.is_dir() {
            continue;
        }
        let script_name = script_file
            .file_name()
            .to_str()
            .ok_or("Parse Error")?
            .to_owned();
        content.scripts.push(script_name);
    }

    response.content = Some(content);
    let response = serde_json::to_string(&response).unwrap();
    Ok(response)
}

pub async fn tests(
    project_id: &str,
    script_id: &str,
    red_client: Data<&redis::Client>,
) -> Result<String, Box<dyn Error>> {
    let mut response = shared::models::http::Response {
        success: true,
        message: "Tests",
        error: None,
        content: None,
    };
    let mut red_connection = red_client.get_connection().unwrap();
    let running_tests: HashSet<String> =
        if let Ok(set) = red_connection.smembers(shared::RUNNING_TESTS) {
            set
        } else {
            HashSet::new()
        };
    let mut content = shared::models::http::tests::Content { tests: Vec::new() };
    let script_dir =
        match std::fs::read_dir(shared::get_a_script_results_dir(project_id, script_id)) {
            Ok(dir) => dir,
            Err(_) => {
                response.content = Some(content);
                let response = serde_json::to_string(&response).unwrap();
                return Ok(response);
            }
        };
    for test_dir in script_dir {
        let test_id = test_dir?
            .file_name()
            .to_str()
            .ok_or("Parse Error")?
            .to_owned();
        //get results
        let results = shared::get_results(project_id, script_id, &test_id);
        let task_id = shared::encode_test_id(project_id, script_id, &test_id);
        let mut status = 1;
        if running_tests.contains(&task_id) {
            status = 0;
        }
        //get info
        let info = shared::get_info(project_id, script_id, &test_id);
        //get history
        let history = shared::get_results_history(project_id, script_id, &test_id);
        content.tests.push(shared::models::Test {
            id: test_id,
            project_id: project_id.to_owned(),
            script_id: script_id.to_owned(),
            status: status,
            results: results,
            history: history,
            info: info,
        });
    }
    response.content = Some(content);
    let response = serde_json::to_string(&response).unwrap();
    Ok(response)
}

pub async fn stop_test(
    project_id: String,
    script_id: String,
    test_id: String,
    subscriptions: Data<&Arc<RwLock<HashMap<String, (u32, Sender<String>)>>>>,
) -> Result<String, Box<dyn Error>> {
    let ip =
        shared::get_worker_ip(&project_id, &script_id, &test_id).ok_or("No worker ip found")?;
    let client = reqwest::Client::new();
    let response = client
        .post(&format!(
            "http://{}/stop_test/{}/{}/{}",
            ip, project_id, script_id, test_id
        ))
        .send()
        .await?;
    {
        let script_id = shared::encode_script_id(&project_id, &script_id);
        let subscriptions_guard = subscriptions.read();
        if let Some((_, sender)) = subscriptions_guard.get(&script_id) {
            //create event
            let websocket_message = models::websocket::WebSocketMessage {
                event_type: "TEST_STOPPED",
                event: models::websocket::tests::TestStoppeddEvent { id: test_id },
            };
            if sender
                .send(serde_json::to_string(&websocket_message).unwrap())
                .is_err()
            {
                println!("STOP TEST: No clients are connected!");
            }
        }
    }
    Ok(response.text().await.unwrap())
}

pub async fn delete_test(
    project_id: String,
    script_id: String,
    test_id: String,
    subscriptions: Data<&Arc<RwLock<HashMap<String, (u32, Sender<String>)>>>>,
    red_client: Data<&redis::Client>,
) -> Result<String, Box<dyn Error>> {
    //check if test is running
    let mut red_connection = red_client.get_connection().unwrap();
    let running_tests: std::collections::HashSet<String> =
        if let Ok(set) = red_connection.smembers(shared::RUNNING_TESTS) {
            set
        } else {
            std::collections::HashSet::new()
        };
    if !running_tests.contains(&shared::encode_test_id(&project_id, &script_id, &test_id)) {
        let mut response = models::http::Response::<String> {
            success: true,
            message: "Test delete",
            error: None,
            content: None,
        };
        if shared::delete_test(&project_id, &script_id, &test_id).is_err() {
            response.success = false;
            response.error = Some("Could not delete test");
        }
        return Ok(serde_json::to_string(&response).unwrap());
    }
    //TODO! if worker is not responding?
    let ip =
        shared::get_worker_ip(&project_id, &script_id, &test_id).ok_or("No worker ip found")?;
    let client = reqwest::Client::new();
    let response = client
        .post(&format!(
            "http://{}/delete_test/{}/{}/{}",
            ip, project_id, script_id, test_id
        ))
        .send()
        .await?;
    {
        let script_id = shared::encode_script_id(&project_id, &script_id);
        let subscriptions_guard = subscriptions.read();
        if let Some((_, sender)) = subscriptions_guard.get(&script_id) {
            //create event
            let websocket_message = models::websocket::WebSocketMessage {
                event_type: "TEST_DELETED",
                event: models::websocket::tests::TestDeletedEvent { id: test_id },
            };
            if sender
                .send(serde_json::to_string(&websocket_message).unwrap())
                .is_err()
            {
                println!("DELETE TEST: No clients are connected!");
            }
        }
    }
    Ok(response.text().await.unwrap())
}
