use parking_lot::RwLock;
use poem::web::{Data, Json, Multipart};
use redis::Commands;
use shared::models;
use std::error::Error;
use std::io::Write;
use std::{
    collections::{HashMap, HashSet},
    fs::canonicalize,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    str,
    sync::{Arc, Mutex},
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
                eprintln!(
                    "[{}] [{}] Error reading from stream: {}",
                    shared::get_date_and_time(),
                    line!(),
                    err
                );
                break;
            }
            Ok(got) => {
                if got == 0 {
                    break;
                } else if got == 1 {
                    vec.push(buf[0])
                } else {
                    eprintln!(
                        "[{}] [{}] Unexpected number of bytes: {}",
                        shared::get_date_and_time(),
                        line!(),
                        got
                    );
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
    currently_installing_projects: Data<&Arc<Mutex<bool>>>,
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
        if !exists {
            let full_file_name = shared::get_temp_dir().join(file_name);
            let full_file_name_prefix = full_file_name.parent().ok_or("Upload Error")?;
            std::fs::create_dir_all(full_file_name_prefix)?;
            let mut file = std::fs::File::create(full_file_name)?;
            if let Ok(bytes) = field.bytes().await {
                file.write(&bytes)?;
            }
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
        if let Ok(cmd_) = Command::new("cmd")
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
            .spawn()
        {
            cmd_
        } else {
            std::fs::remove_dir_all(project_temp_dir)?;
            response.error = Some("System Error");
            response.success = false;
            return Ok(serde_json::to_string(&response).unwrap());
        }
    } else {
        let pip_location_linux = Path::new(&env_dir).join("bin").join("pip3");
        if let Ok(cmd_) = Command::new("bash")
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
            .spawn()
        {
            cmd_
        } else {
            std::fs::remove_dir_all(project_temp_dir)?;
            response.error = Some("System Error");
            response.success = false;
            return Ok(serde_json::to_string(&response).unwrap());
        }
    };
    let mut installing_tasks_guard = installing_tasks.write();
    let project_name = project_temp_dir.file_name().ok_or("Upload Error")?;

    installing_tasks_guard.insert(project_name.to_str().ok_or("Upload Error")?.to_owned(), cmd);
    // run the thread
    let main_sender = main_sender.clone();
    if let Ok(mut currently_installing_projects_mutex) = currently_installing_projects.lock() {
        if !*currently_installing_projects_mutex {
            *currently_installing_projects_mutex = true;
            println!(
                "[{}] PROJECTS GARBAGE COLLECTOR: Running!",
                shared::get_date_and_time()
            );
            let tokio_currently_installing_projects = currently_installing_projects.clone();
            let tokio_installing_tasks = Arc::clone(&installing_tasks);
            tokio::spawn(async move {
                loop {
                    let mut to_be_deleted: Vec<String> = Vec::new();
                    let mut installing_projects: Vec<models::websocket::projects::Project> =
                        Vec::new();
                    {
                        let mut tokio_tasks_guard = tokio_installing_tasks.write();
                        if tokio_tasks_guard.len() < 1 {
                            if let Ok(mut lock) = tokio_currently_installing_projects.lock() {
                                *lock = false;
                                println!(
                                    "[{}] PROJECTS GARBAGE COLLECTOR: Terminating!",
                                    shared::get_date_and_time()
                                );
                            } else {
                                eprintln!(
                                    "[{}] ERROR: PROJECTS GARBAGE COLLECTOR: failed to lock",
                                    shared::get_date_and_time()
                                );
                            }
                            break;
                        }
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
                                            println!("[{}] PROJECTS GARBAGE COLLECTOR: Project [{}] terminated with code [{}]!", shared::get_date_and_time(), id, code);
                                            if code != 0 {
                                                if let Some(stderr) = cmd.stderr.take() {
                                                    let err = child_stream_to_vec(stderr);
                                                    if let Ok(error_string) = str::from_utf8(&err) {
                                                        to_be_deleted.push(id.to_owned());
                                                        project.error =
                                                            Some(error_string.to_owned());
                                                        println!("[{}] PROJECTS GARBAGE COLLECTOR: Project [{}] terminated with error:\n{:?}", shared::get_date_and_time(), id, error_string);
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
                                                        println!("[{}] PROJECTS GARBAGE COLLECTOR: Project [{}] moved to installed projects!", shared::get_date_and_time(), id);
                                                    }
                                                    Err(e) => {
                                                        eprintln!("[{}] ERROR: PROJECTS GARBAGE COLLECTOR: Project [{}] failed to move to installed projects!\n{:?}", shared::get_date_and_time(), id, e);
                                                    }
                                                }
                                            }
                                        }
                                        None => {
                                            println!("[{}] PROJECTS GARBAGE COLLECTOR: Project [{}] terminated by signal!", shared::get_date_and_time(), id);
                                        }
                                    }
                                }
                                Ok(None) => {
                                    project.status = 0; // process is running
                                }
                                Err(e) => {
                                    eprintln!("[{}] ERROR: PROJECTS GARBAGE COLLECTOR: Project [{}]: could not wait on child process error: {:?}", shared::get_date_and_time(), id, e);
                                }
                            }
                            installing_projects.push(project);
                        }
                        //remove finished
                        for id in to_be_removed.iter() {
                            tokio_tasks_guard.remove_entry(id);
                            println!(
                                "[{}] PROJECTS GARBAGE COLLECTOR: Project [{}] removed!",
                                shared::get_date_and_time(),
                                id
                            );
                        }
                    }
                    //delete not valid
                    for id in to_be_deleted.iter() {
                        match std::fs::remove_dir_all(shared::get_a_temp_dir(id)) {
                            Ok(_) => (),
                            Err(e) => {
                                eprintln!("[{}] ERROR: PROJECTS GARBAGE COLLECTOR: Project [{}]: folder could not be deleted!\n{:?}",shared::get_date_and_time(),  id, e);
                            }
                        };
                        match std::fs::remove_dir_all(shared::get_an_environment_dir(id)) {
                            Ok(_) => (),
                            Err(e) => {
                                eprintln!("[{}] ERROR: PROJECTS GARBAGE COLLECTOR: Project [{}]: environment could not be deleted!\n{:?}",shared::get_date_and_time(),  id, e);
                            }
                        };
                        println!(
                            "[{}] PROJECTS GARBAGE COLLECTOR: Project [{}] deleted!",
                            shared::get_date_and_time(),
                            id
                        );
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
                        println!(
                            "[{}] PROJECTS GARBAGE COLLECTOR: No clients are connected!",
                            shared::get_date_and_time()
                        );
                    }
                    sleep(Duration::from_secs(3)).await;
                }
            });
        } else {
            println!(
                "[{}] PROJECTS GARBAGE COLLECTOR: Already running!",
                shared::get_date_and_time()
            );
        }
    } else {
        eprintln!(
            "[{}] ERROR: MASTER: Project [{:#?}] failed to lock",
            shared::get_date_and_time(),
            project_name
        );
        Err("Could not lock. System error")?;
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
            let metadata = script_file.metadata()?;
            if metadata.is_dir() {
                continue;
            }
            let script_name = script_file
                .file_name()
                .to_str()
                .ok_or("Parse Error")?
                .to_owned();
            let extension = Path::new(&script_name).extension();
            if let Some(extension) = extension {
                if extension != "py" {
                    continue;
                }
            }
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
        let metadata = script_file.metadata()?;
        if metadata.is_dir() {
            continue;
        }
        let script_name = script_file
            .file_name()
            .to_str()
            .ok_or("Parse Error")?
            .to_owned();
        let extension = Path::new(&script_name).extension();
        if let Some(extension) = extension {
            if extension != "py" {
                continue;
            }
        }
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
    let mut red_connection;
    if let Ok(connection) = red_client.get_connection() {
        red_connection = connection;
    } else {
        response.success = false;
        response.error = Some("Could not connect to database");
        return Ok(serde_json::to_string(&response).unwrap());
    }
    let running_tests: HashSet<String> =
        if let Ok(set) = red_connection.smembers(shared::RUNNING_TESTS) {
            set
        } else {
            HashSet::new()
        };
    let mut content = shared::models::http::tests::Content {
        tests: Vec::new(),
        config: shared::get_config(&project_id, &script_id),
    };
    let script_dir =
        match std::fs::read_dir(shared::get_a_script_results_dir(project_id, script_id)) {
            Ok(dir) => dir,
            Err(_) => {
                response.success = false;
                response.error = Some("Could not read a directory");
                let response = serde_json::to_string(&response).unwrap();
                return Ok(response);
            }
        };
    for test_dir in script_dir {
        let test_dir = test_dir?;
        if test_dir.metadata()?.is_file() {
            continue;
        }
        let test_id = test_dir
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

pub async fn all_running_tests(red_client: Data<&redis::Client>) -> Result<String, Box<dyn Error>> {
    let mut response = shared::models::http::Response {
        success: true,
        message: "Tests",
        error: None,
        content: None,
    };
    let mut red_connection;
    if let Ok(connection) = red_client.get_connection() {
        red_connection = connection;
    } else {
        response.success = false;
        response.error = Some("Could not connect to database");
        return Ok(serde_json::to_string(&response).unwrap());
    }
    let running_tests: HashSet<String> =
        if let Ok(set) = red_connection.smembers(shared::RUNNING_TESTS) {
            set
        } else {
            HashSet::new()
        };
    let mut content = shared::models::http::tests::Content {
        tests: Vec::new(),
        config: None,
    };
    for running_test in running_tests {
        let (project_id, script_id, test_id) = shared::decode_test_id(&running_test);
        //get results
        let results = shared::get_results(&project_id, &script_id, &test_id);
        let status = 0;
        //get info
        let info = shared::get_info(&project_id, &script_id, &test_id);
        //get history
        let history = shared::get_results_history(&project_id, &script_id, &test_id);
        content.tests.push(shared::models::Test {
            id: test_id.to_owned(),
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
                event_type: shared::TEST_STOPPED,
                event: models::websocket::tests::TestStoppeddEvent { id: test_id },
            };
            if sender
                .send(serde_json::to_string(&websocket_message).unwrap())
                .is_err()
            {
                println!(
                    "[{}] STOP TEST: No clients are connected!",
                    shared::get_date_and_time()
                );
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
    let mut response = models::http::Response::<String> {
        success: true,
        message: "Test delete",
        error: None,
        content: None,
    };
    //check if test is running
    let running_tests: std::collections::HashSet<String>;
    if let Ok(mut connection) = red_client.get_connection() {
        if let Ok(set) = connection.smembers(shared::RUNNING_TESTS) {
            running_tests = set;
        } else {
            response.success = false;
            response.error = Some("Could not connect to database");
            return Ok(serde_json::to_string(&response).unwrap());
        };
    } else {
        response.success = false;
        response.error = Some("Could not connect to database");
        return Ok(serde_json::to_string(&response).unwrap());
    }
    if !running_tests.contains(&shared::encode_test_id(&project_id, &script_id, &test_id)) {
        if shared::delete_test(&project_id, &script_id, &test_id).is_err() {
            response.success = false;
            response.error = Some("Could not delete test");
        }
        return Ok(serde_json::to_string(&response).unwrap());
    }
    let ip =
        shared::get_worker_ip(&project_id, &script_id, &test_id).ok_or("No worker ip found")?;
    let client = reqwest::Client::new();
    match client
        .post(&format!(
            "http://{}/delete_test/{}/{}/{}",
            ip, project_id, script_id, test_id
        ))
        .send()
        .await
    {
        Ok(response) => {
            {
                let script_id = shared::encode_script_id(&project_id, &script_id);
                let subscriptions_guard = subscriptions.read();
                if let Some((_, sender)) = subscriptions_guard.get(&script_id) {
                    //create event
                    let websocket_message = models::websocket::WebSocketMessage {
                        event_type: shared::TEST_DELETED,
                        event: models::websocket::tests::TestDeletedEvent { id: test_id },
                    };
                    if sender
                        .send(serde_json::to_string(&websocket_message).unwrap())
                        .is_err()
                    {
                        println!(
                            "[{}] MASTER: DELETE TEST: No clients are connected!",
                            shared::get_date_and_time()
                        );
                    }
                }
            }
            return Ok(response.text().await.unwrap());
        }
        Err(e) => {
            eprintln!(
                "[{}] MASTER: DELETE TEST: Could not connect to worker [{}],\n{}",
                shared::get_date_and_time(),
                ip,
                e
            );
            response.success = false;
            response.error = Some("Could not connect to worker");
            Ok(serde_json::to_string(&response).unwrap())
        }
    }
}

pub async fn stop_script(
    project_id: &str,
    script_id: &str,
    red_client: Data<&redis::Client>,
) -> Result<String, Box<dyn Error>> {
    let mut response = models::http::Response::<HashMap<&str, String>> {
        success: true,
        message: "Script stop",
        error: None,
        content: None,
    };
    let mut error = String::new();
    let mut contents: HashMap<&str, String> = HashMap::new();
    let workers: std::collections::HashSet<String>;
    if let Ok(mut connection) = red_client.get_connection() {
        if let Ok(set) = connection.smembers(shared::REGISTERED_WORKERS) {
            workers = set;
        } else {
            response.success = false;
            response.error = Some("Could not connect to database");
            return Ok(serde_json::to_string(&response).unwrap());
        };
    } else {
        response.success = false;
        response.error = Some("Could not connect to database");
        return Ok(serde_json::to_string(&response).unwrap());
    }
    let script_id_enc = shared::encode_script_id(project_id, script_id);
    println!(
        "[{}] MASTER: STOP SCRIPT ATTEMPT: [{}]",
        shared::get_date_and_time(),
        script_id_enc
    );
    for worker in workers.iter() {
        let client = reqwest::Client::new();
        if let Ok(response) = client
            .post(&format!(
                "http://{}/stop_script/{}/{}",
                worker, project_id, script_id
            ))
            .send()
            .await
        {
            let res = response.text().await.unwrap();
            println!(
                "[{}] MASTER: STOP SCRIPT [{}]: Worker [{}] response: [{}]",
                shared::get_date_and_time(),
                script_id_enc,
                worker,
                res
            );
            contents.insert(worker, res);
        } else {
            eprintln!(
                "[{}] MASTER: STOP SCRIPT [{}]: Worker [{}]: Could not connect to worker!",
                shared::get_date_and_time(),
                script_id_enc,
                worker
            );
            error.push_str(&format!("Could not connect to worker [{}]\n", worker));
            response.success = false;
            contents.insert(worker, "Could not connect to worker".to_owned());
        }
    }
    if !response.success {
        response.error = Some(&error);
    }
    response.content = Some(contents);
    Ok(serde_json::to_string(&response).unwrap())
}

pub async fn stop_project<'a>(
    project_id: &str,
    workers: &HashSet<String>,
    error: &'a mut String,
) -> models::http::Response<'a, HashMap<String, String>> {
    let mut response = models::http::Response::<HashMap<String, String>> {
        success: true,
        message: "Project stop",
        error: None,
        content: None,
    };
    let mut contents: HashMap<String, String> = HashMap::new();
    println!(
        "[{}] MASTER: STOP PROJECT ATTEMPT: [{}]",
        shared::get_date_and_time(),
        project_id
    );
    for worker in workers.iter() {
        let client = reqwest::Client::new();
        if let Ok(res) = client
            .post(&format!("http://{}/stop_project/{}", worker, project_id))
            .send()
            .await
        {
            let res = res.text().await.unwrap();
            let de_res: models::http::Response<HashMap<String, bool>> =
                serde_json::from_str(&res).unwrap();
            println!(
                "[{}] MASTER: STOP PROJECT [{}]: Worker [{}] response: [{}]",
                shared::get_date_and_time(),
                project_id,
                worker,
                res
            );
            if !de_res.success {
                error.push_str(&format!("{}\n", de_res.error.unwrap()));
                response.success = false;
            }
            contents.insert(worker.to_owned(), res);
        } else {
            eprintln!(
                "[{}] MASTER: STOP PROJECT [{}]: Worker [{}]: Could not connect to worker!",
                shared::get_date_and_time(),
                project_id,
                worker
            );
            error.push_str(&format!("Could not connect to worker [{}]\n", worker));
            response.success = false;
            contents.insert(worker.to_owned(), "Could not connect to worker".to_owned());
        }
    }
    if !response.success {
        response.error = Some(error);
    }
    response.content = Some(contents);
    return response;
}

pub async fn delete_projects(
    projects_to_be_deleted: Json<models::http::projects::ProjectIds>,
    red_client: Data<&redis::Client>,
    main_sender: Data<&tokio::sync::broadcast::Sender<String>>,
) -> Result<String, Box<dyn Error>> {
    let mut response = models::http::Response::<HashMap<String, (bool, String)>> {
        success: true,
        message: "Delete projects",
        error: None,
        content: None,
    };
    let mut contents: HashMap<String, (bool, String)> = HashMap::new();
    let workers: std::collections::HashSet<String>;
    let mut red_connection;
    if let Ok(connection) = red_client.get_connection() {
        red_connection = connection;
    } else {
        response.success = false;
        response.error = Some("Could not connect to database");
        return Ok(serde_json::to_string(&response).unwrap());
    }
    if let Ok(set) = red_connection.smembers(shared::REGISTERED_WORKERS) {
        workers = set;
    } else {
        response.success = false;
        response.error = Some("Could not connect to database");
        return Ok(serde_json::to_string(&response).unwrap());
    };
    for project_id in projects_to_be_deleted.project_ids.iter() {
        //if project is allready locked continue
        let locked_projects: std::collections::HashSet<String>;
        if let Ok(set) = red_connection.smembers(shared::LOCKED_PROJECTS) {
            locked_projects = set;
        } else {
            response.success = false;
            response.error = Some("Could not connect to database");
            return Ok(serde_json::to_string(&response).unwrap());
        };
        if locked_projects.contains(project_id) {
            continue;
        }
        //lock project
        if red_connection
            .sadd::<_, _, ()>(shared::LOCKED_PROJECTS, &projects_to_be_deleted.project_ids)
            .is_err()
        {
            response.success = false;
            response.error = Some("Could not connect to database");
            return Ok(serde_json::to_string(&response).unwrap());
        }
        //stop project
        let mut stop_project_error = String::new();
        let stop_response = stop_project(&project_id, &workers, &mut stop_project_error).await;
        if stop_response.success {
            //delete project if all tests are stopped
            let mut delete_project_error = String::new();
            let delete_response = delete_project(&project_id, &mut delete_project_error);
            if delete_response.success {
                contents.insert(
                    project_id.to_owned(),
                    (true, delete_project_error.to_owned()),
                );
                //notify browser
                let websocket_message = models::websocket::WebSocketMessage {
                    event_type: shared::PROJECT_DELETED,
                    event: models::websocket::projects::DeletedProject {
                        id: project_id.to_owned(),
                    },
                };
                if main_sender
                    .send(serde_json::to_string(&websocket_message).unwrap())
                    .is_err()
                {
                    println!(
                        "[{}] DELETE PROJECT EVENT: No clients are connected!",
                        shared::get_date_and_time()
                    );
                }
            } else {
                response.success = false;
                contents.insert(
                    project_id.to_owned(),
                    (false, delete_project_error.to_owned()),
                );
            }
        } else {
            response.success = false;
            contents.insert(
                project_id.to_owned(),
                (false, stop_project_error.to_owned()),
            );
        }
        //unlock project
        let _: () = red_connection
            .srem(shared::LOCKED_PROJECTS, &projects_to_be_deleted.project_ids)
            .unwrap_or_default();
    }
    response.content = Some(contents);
    Ok(serde_json::to_string(&response).unwrap())
}

fn delete_project<'a>(
    project_id: &str,
    error: &'a mut String,
) -> models::http::Response<'a, HashMap<String, String>> {
    let mut response = models::http::Response::<HashMap<String, String>> {
        success: true,
        message: "Delete project",
        error: None,
        content: None,
    };
    let project_dir = shared::get_a_project_dir(project_id);
    let env_dir = shared::get_an_environment_dir(project_id);
    let results_dir = shared::get_a_project_results_dir(project_id);
    if results_dir.exists() {
        // match std::fs::remove_dir_all(&results_dir) {
        //     Ok(_) => {
        //         println!(
        //             "[{}] MASTER: DELETE PROJECT [{}]: results directory deleted!",
        //             shared::get_date_and_time(),
        //             project_id,
        //         );
        //     }
        //     Err(e) => {
        //         eprintln!(
        //             "[{}] MASTER: DELETE PROJECT [{}]: Could not delete results directory: {}\n",
        //             shared::get_date_and_time(),
        //             project_id,
        //             e
        //         );
        //         error.push_str("Could not delete results directory\n");
        //         response.success = false;
        //     }
        // }
    }
    if env_dir.exists() {
        match std::fs::remove_dir_all(&env_dir) {
            Ok(_) => {
                println!(
                    "[{}] MASTER: DELETE PROJECT [{}]: environment directory deleted!",
                    shared::get_date_and_time(),
                    project_id,
                );
            }
            Err(e) => {
                eprintln!(
                    "[{}] MASTER: DELETE PROJECT [{}]: Could not delete environment directory: {}\n",
                    shared::get_date_and_time(),
                    project_id,
                    e
                );
                error.push_str("Could not delete environment directory\n");
                response.success = false;
            }
        }
    }
    if project_dir.exists() {
        match std::fs::remove_dir_all(&project_dir) {
            Ok(_) => {
                println!(
                    "[{}] MASTER: DELETE PROJECT [{}]: project directory deleted!",
                    shared::get_date_and_time(),
                    project_id,
                );
            }
            Err(e) => {
                eprintln!(
                    "[{}] MASTER: DELETE PROJECT [{}]: Could not delete project directory: {}\n",
                    shared::get_date_and_time(),
                    project_id,
                    e
                );
                error.push_str("Could not delete project directory\n");
                response.success = false;
            }
        }
    }
    if !response.success {
        response.error = Some(error);
    }
    return response;
}

pub fn check_script<'a>(project_id: &'a str, script_id: &'a str) -> Result<String, Box<dyn Error>> {
    let mut response = models::http::Response::<String> {
        success: true,
        message: "Test check",
        error: None,
        content: None,
    };
    let locust_file = shared::get_a_locust_dir(project_id).join(script_id);

    if !locust_file.exists() {
        response.error = Some("Script not found");
        response.success = false;
        return Ok(serde_json::to_string(&response).unwrap());
    }

    let env_dir = shared::get_an_environment_dir(&project_id);
    let can_locust_file = canonicalize(&locust_file).unwrap(); //absolute path for commands current dir

    let mut cmd = if cfg!(target_os = "windows") {
        let args = vec![
            "-f",
            can_locust_file.to_str().ok_or("Run Error")?,
            "--headless",
            "--users",
            "1",
            "--spawn-rate",
            "1",
            "--run-time",
            "3s",
            "--host",
            "http://localhost:6000",
        ];
        Command::new(Path::new(&env_dir).join("Scripts").join("locust.exe"))
            .current_dir(shared::get_a_project_dir(&project_id))
            .args(&args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped())
            .spawn()?
    } else {
        //linux
        let can_locust_location_linux =
            canonicalize(Path::new(&env_dir).join("bin").join("locust")).unwrap();
        let command = format!(
            "{} -f {} --headless --users 1 --spawn-rate 1 --run-time 3s --host http://localhost:6000",
            can_locust_location_linux.to_str().ok_or("Run Error")?,
            can_locust_file.to_str().ok_or("Run Error")?,
        );
        Command::new("bash")
            .current_dir(shared::get_a_project_dir(&project_id))
            .args(&["-c", &command])
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped())
            .spawn()?
    };

    cmd.wait()?;

    //locust prints everything to stderr :)
    let mut content_string = String::new();
    if let Some(stderr) = cmd.stderr.take() {
        let err = child_stream_to_vec(stderr);
        if let Ok(error_string) = str::from_utf8(&err) {
            content_string.push_str(error_string);
        }
    }

    response.content = Some(content_string);

    return Ok(serde_json::to_string(&response).unwrap());
}

pub fn preview_script(project_id: &str, script_id: &str) -> Result<String, Box<dyn Error>> {
    let script_content = shared::read_script_content(project_id, script_id);
    let response = models::http::Response::<String> {
        success: true,
        message: "Test preview",
        error: None,
        content: script_content,
    };
    return Ok(serde_json::to_string(&response).unwrap());
}
