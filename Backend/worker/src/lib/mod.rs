use crate::models;
pub mod task;
use parking_lot::RwLock;
use poem::web::{Data, Json, Multipart};
use std::error::Error;
use std::fs::canonicalize;
use std::io::{Read, Write};

use std::path::PathBuf;
use std::process::Child;
use std::str;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};
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

pub async fn start_test(
    project_id: &str,
    script_id: &str,
    mut req: Json<models::http::TestInfo>,
    running_tests: Data<&Arc<RwLock<HashMap<String, task::Task>>>>,
    currently_running_tests: Data<&Arc<Mutex<bool>>>,
    wanted_scripts: Data<&Arc<RwLock<HashSet<String>>>>,
    id: String,
    task_id: String,
) -> Result<String, Box<dyn Error>> {
    //let workers = req.workers.unwrap_or(1);
    let mut response = models::http::Response {
        success: true,
        message: "Test start",
        error: None,
        content: None,
    };

    let locust_file = shared::get_a_locust_dir(project_id).join(script_id);

    //checking here if the locust file exists and then we will check again before running if the script was in the meantime deleted
    if !locust_file.exists() {
        response.error = Some("Script not found");
        response.success = false;
        return Ok(serde_json::to_string(&response).unwrap());
    }

    //create test dir
    let test_dir = shared::get_a_test_results_dir(project_id, script_id, &id);
    std::fs::create_dir_all(&test_dir)?;

    //define paths
    let env_dir = shared::get_an_environment_dir(&project_id);
    let can_locust_file = canonicalize(&locust_file).unwrap(); //absolute path for commands current dir
    let log_file_relative_path = shared::get_log_file_relative_path(project_id, script_id, &id);
    let csv_file_relative_path = shared::get_csv_file_relative_path(project_id, script_id, &id);

    //create commands
    let users_command = if let Some(req_users) = &req.users {
        format!("--users {}", req_users)
    } else {
        String::from("--users 1")
    };
    let spawn_rate_command = if let Some(req_spawn_rate) = &req.spawn_rate {
        format!("--spawn-rate {}", req_spawn_rate)
    } else {
        String::from("--spawn-rate 1")
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
    let workers = if let Some(req_workers) = req.workers {
        if req_workers < 1 {
            0
        } else {
            req_workers
        }
    } else {
        0
    };

    let mut running_tests_guard = running_tests.write();
    //checking if the script was not deleted in the meantime after performing the lock
    if !locust_file.exists() {
        //delete test dir
        std::fs::remove_dir_all(&test_dir)?;
        response.error = Some("Script was deleted!");
        response.success = false;
        return Ok(serde_json::to_string(&response).unwrap());
    }

    //run
    let cmd = if cfg!(target_os = "windows") {
        let mut args = Vec::new();
        args.push("-f");
        args.push(can_locust_file.to_str().ok_or("Run Error")?);
        args.push("--headless");

        let mut users_command_splitted = users_command.split(" ");
        let mut spawn_rate_command_splitted = spawn_rate_command.split(" ");
        let mut log_command_splitted = log_command.split(" ");
        let mut csv_command_splitted = csv_command.split(" ");

        args.push(users_command_splitted.next().unwrap());
        args.push(users_command_splitted.next().unwrap());
        args.push(spawn_rate_command_splitted.next().unwrap());
        args.push(spawn_rate_command_splitted.next().unwrap());
        args.push(log_command_splitted.next().unwrap());
        args.push(log_command_splitted.next().unwrap());
        args.push(csv_command_splitted.next().unwrap());
        args.push(csv_command_splitted.next().unwrap());

        if !time_command.is_empty() {
            let mut time_command_splitted = time_command.split(" ");
            args.push(time_command_splitted.next().unwrap());
            args.push(time_command_splitted.next().unwrap());
        }

        if !host_command.is_empty() {
            let mut host_command_splitted = host_command.split(" ");
            args.push(host_command_splitted.next().unwrap());
            args.push(host_command_splitted.next().unwrap());
        }

        if workers > 0 {
            let port;
            if let Ok(port_) = shared::get_a_free_port() {
                port = port_;
            } else {
                //delete test dir
                std::fs::remove_dir_all(&test_dir)?;
                response.error = Some("Could not get a free port");
                response.success = false;
                return Ok(serde_json::to_string(&response).unwrap());
            }
            println!(
                "[{}] WORKER: Starting master on port [{}] with [{}] workers",
                shared::get_date_and_time(),
                port,
                workers
            );
            let mut children = Vec::with_capacity(workers as usize);
            for i in 0..workers {
                let log_file_relative_path_for_worker =
                    shared::get_log_file_relative_path_for_worker(
                        project_id,
                        script_id,
                        &id,
                        i + 1,
                    );
                let mut worker_args = Vec::new();
                worker_args.push("-f");
                worker_args.push(can_locust_file.to_str().ok_or("Run Error")?);
                worker_args.push("--logfile");
                worker_args.push(
                    log_file_relative_path_for_worker
                        .to_str()
                        .ok_or("Run Error")?,
                );
                worker_args.push("--worker");
                let port_command = format!("--master-port={}", port);
                worker_args.push(&port_command);

                children.push(
                    Command::new(Path::new(&env_dir).join("Scripts").join("locust.exe"))
                        .current_dir(shared::get_a_project_dir(&project_id))
                        .args(&worker_args)
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        // .stdout(Stdio::inherit())
                        // .stderr(Stdio::inherit())
                        .spawn()?,
                );
            }
            args.push("--master");
            let port_command = format!("--master-bind-port={}", port);
            args.push(&port_command);
            args.push("--expect-workers");
            let workers_str = workers.to_string();
            args.push(&workers_str);

            task::Task::MasterTask(
                Command::new(Path::new(&env_dir).join("Scripts").join("locust.exe"))
                    .current_dir(shared::get_a_project_dir(&project_id))
                    .args(&args)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    // .stdout(Stdio::inherit())
                    // .stderr(Stdio::inherit())
                    .spawn()?,
                children,
                task_id.clone(),
            )
        } else {
            task::Task::NormalTask(
                Command::new(Path::new(&env_dir).join("Scripts").join("locust.exe"))
                    .current_dir(shared::get_a_project_dir(&project_id))
                    .args(&args)
                    //.stdout(Stdio::null())
                    //.stderr(Stdio::null())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()?,
                task_id.clone(),
            )
        }
    } else {
        //linux
        let can_locust_location_linux =
            canonicalize(Path::new(&env_dir).join("bin").join("locust")).unwrap();
        let command = format!(
            "{} -f {} --headless {} {} {} {} {} {}",
            can_locust_location_linux.to_str().ok_or("Run Error")?,
            can_locust_file.to_str().ok_or("Run Error")?,
            users_command,
            spawn_rate_command,
            time_command,
            host_command,
            log_command,
            csv_command,
        );
        if workers > 0 {
            let mut enable_worker_id = false;
            let config = shared::get_config(&project_id, &script_id);
            if let Some(config) = config {
                if let Some(enb) = config.enable_worker_id {
                    enable_worker_id = enb;
                }
            }

            let port;
            if let Ok(port_) = shared::get_a_free_port() {
                port = port_;
            } else {
                //delete test dir
                std::fs::remove_dir_all(&test_dir)?;
                response.error = Some("Could not get a free port");
                response.success = false;
                return Ok(serde_json::to_string(&response).unwrap());
            }
            println!(
                "[{}] WORKER: Starting master on port [{}] with [{}] workers",
                shared::get_date_and_time(),
                port,
                workers
            );
            let mut children = Vec::with_capacity(workers as usize);
            for i in 0..workers {
                let worker_id_flag = if enable_worker_id {
                    format!("--worker-id={}", i + 1)
                } else {
                    String::new()
                };

                let log_file_relative_path_for_worker =
                    shared::get_log_file_relative_path_for_worker(
                        project_id,
                        script_id,
                        &id,
                        i + 1,
                    );
                children.push(
                    Command::new("bash")
                        .current_dir(shared::get_a_project_dir(&project_id))
                        .args(&[
                            "-c",
                            &format!(
                                "{} -f {} --logfile {} --worker --master-port={} {}",
                                can_locust_location_linux.to_str().ok_or("Run Error")?,
                                can_locust_file.to_str().ok_or("Run Error")?,
                                log_file_relative_path_for_worker
                                    .to_str()
                                    .ok_or("Run Error")?,
                                port,
                                worker_id_flag
                            ),
                        ])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        // .stdout(Stdio::inherit())
                        // .stderr(Stdio::inherit())
                        .spawn()?,
                );
            }
            task::Task::MasterTask(
                Command::new("bash")
                    .current_dir(shared::get_a_project_dir(&project_id))
                    .args(&[
                        "-c",
                        &format!(
                            "{} --master --master-bind-port={} --expect-workers {}",
                            command, port, workers
                        ),
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    // .stdout(Stdio::inherit())
                    // .stderr(Stdio::inherit())
                    .spawn()?,
                children,
                task_id.clone(),
            )
        } else {
            task::Task::NormalTask(
                Command::new("bash")
                    .current_dir(shared::get_a_project_dir(&project_id))
                    .args(&["-c", &command])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    // .stdout(Stdio::inherit())
                    // .stderr(Stdio::inherit())
                    .spawn()?,
                task_id.clone(),
            )
        }
    };
    //save test info
    let test_info = shared::models::http::TestInfo {
        project_id: Some(project_id.to_string()),
        script_id: Some(script_id.to_string()),
        users: std::mem::take(&mut req.users),
        spawn_rate: std::mem::take(&mut req.spawn_rate),
        workers: std::mem::take(&mut req.workers),
        host: std::mem::take(&mut req.host),
        time: std::mem::take(&mut req.time),
        description: std::mem::take(&mut req.description),
        id: Some(id.clone()),
        worker_ip: None,
    };
    let mut file = std::fs::File::create(&test_dir.join("info.json"))?;
    file.write(serde_json::to_string(&test_info).unwrap().as_bytes())?;

    running_tests_guard.insert(task_id, cmd);

    let started_test = shared::models::Test {
        id: id,
        project_id: project_id.to_string(),
        script_id: script_id.to_string(),
        status: 0,
        results: None,
        history: None,
        info: Some(test_info),
    };

    //Notify
    let websocket_message = models::websocket::WebSocketMessage {
        event_type: shared::TEST_STARTED,
        event: &started_test,
    };

    //run the garbage collector
    let wanted_scripts = wanted_scripts.clone();
    if let Ok(mut currently_running_tests_mutex) = currently_running_tests.lock() {
        if !*currently_running_tests_mutex {
            *currently_running_tests_mutex = true;
            println!(
                "[{}] SCRIPTS GARBAGE COLLECTOR: Running!",
                shared::get_date_and_time()
            );
            let tokio_currently_running_tests = currently_running_tests.clone();
            let tokio_running_tests = Arc::clone(&running_tests);
            tokio::spawn(async move {
                loop {
                    let mut tests_info_map: HashMap<
                        String,
                        Vec<models::websocket::tests::TestInfo>,
                    > = HashMap::new();
                    {
                        let mut tokio_tests_guard = tokio_running_tests.write();
                        if tokio_tests_guard.len() < 1 {
                            if let Ok(mut lock) = tokio_currently_running_tests.lock() {
                                *lock = false;
                                println!(
                                    "[{}] SCRIPTS GARBAGE COLLECTOR: Terminating!",
                                    shared::get_date_and_time()
                                );
                            } else {
                                eprintln!(
                                    "[{}] ERROR: SCRIPTS GARBAGE COLLECTOR: failed to lock",
                                    shared::get_date_and_time()
                                );
                            }
                            break;
                        }
                        let mut to_be_removed: Vec<String> = Vec::new();

                        for (id, cmd) in tokio_tests_guard.iter_mut() {
                            let (project_id, script_id, test_id) = shared::decode_test_id(id);
                            let global_script_id = shared::get_global_script_id(id);
                            let mut status = 0;

                            match cmd.try_wait() {
                                Ok(Some(exit_status)) => {
                                    // process finished
                                    status = 1; // process finished
                                    to_be_removed.push(id.to_owned());
                                    match exit_status.code() {
                                        Some(code) => {
                                            println!("[{}] SCRIPTS GARBAGE COLLECTOR: Script [{}] terminated with code [{}]!", shared::get_date_and_time(), id, code);
                                            cmd.kill_children();
                                        }
                                        None => {
                                            println!("[{}] SCRIPTS GARBAGE COLLECTOR: Script [{}] terminated by signal!", shared::get_date_and_time(), id);
                                        }
                                    }
                                }
                                Ok(None) => {
                                    status = 0; // process is running
                                }
                                Err(e) => {
                                    eprintln!("[{}] ERROR: SCRIPTS GARBAGE COLLECTOR: Script [{}]: could not wait on child process error: {:?}",shared::get_date_and_time(), id, e);
                                }
                            }
                            {
                                //check if the script is wanted and save results
                                let wanted_scripts_g = wanted_scripts.read();
                                if wanted_scripts_g.contains(global_script_id)
                                    || wanted_scripts_g.contains(shared::CONTROL_SUB_STRING)
                                {
                                    // println!(
                                    //     "[{}] SCRIPT WANTED: {}",
                                    //     shared::get_date_and_time(),
                                    //     global_script_id
                                    // );
                                    let results =
                                        shared::get_results(project_id, script_id, test_id);
                                    let last_history = None;
                                    // let last_history shared::get_last_result_history(project_id, script_id, test_id);
                                    let test_info = models::websocket::tests::TestInfo {
                                        id: test_id.to_owned(),
                                        results,
                                        status,
                                        last_history,
                                    };
                                    if tests_info_map.contains_key(global_script_id) {
                                        tests_info_map
                                            .get_mut(global_script_id)
                                            .unwrap()
                                            .push(test_info);
                                    } else {
                                        tests_info_map
                                            .insert(global_script_id.to_owned(), vec![test_info]);
                                    }
                                }
                            }
                        }
                        //remove finished
                        for id in to_be_removed.iter() {
                            tokio_tests_guard.remove_entry(id);
                            println!(
                                "[{}] SCRIPTS GARBAGE COLLECTOR: Script [{}] removed!",
                                shared::get_date_and_time(),
                                id
                            );
                        }
                    }
                    //Notify
                    for (script_id, tests_info) in tests_info_map.iter() {
                        let websocket_message = models::websocket::WebSocketMessage {
                            event_type: shared::UPDATE_TEST_INFO,
                            event: models::websocket::tests::TestInfoEvent { tests_info },
                        };
                    }
                    sleep(Duration::from_secs(2)).await;
                }
            });
        } else {
            println!(
                "[{}] SCRIPTS GARBAGE COLLECTOR: Already running!",
                shared::get_date_and_time()
            );
        }
    } else {
        eprintln!(
            "[{}] ERROR: WORKER: Test start failed to lock",
            shared::get_date_and_time()
        );
        Err("Could not lock. System error")?;
    }
    response.content = Some(started_test);
    return Ok(serde_json::to_string(&response).unwrap());
}

pub async fn stop_test(
    task_id: &str,
    running_tests: &Data<&Arc<RwLock<HashMap<String, task::Task>>>>,
    /*red_client: Data<&redis::Client>,*/
) -> Result<String, Box<dyn Error>> {
    let mut response = models::http::Response::<String> {
        success: true,
        message: "Test stop",
        error: None,
        content: None,
    };
    let mut running_tests_guard = running_tests.write();
    match running_tests_guard.get_mut(task_id) {
        Some(cmd) => match cmd.kill() {
            Ok(_) => {
                println!(
                    "[{}] TEST KILLED: [{}]!",
                    shared::get_date_and_time(),
                    task_id
                );
                response.message = "Task stopped";
                //running_tests_guard.remove_entry(&task_id);
                //remove from redis
                // let mut red_connection = red_client.get_connection().unwrap();
                // let _: () = red_connection
                //     .srem(shared::RUNNING_TESTS, &task_id)
                //     .unwrap();
            }
            Err(_) => {
                eprintln!(
                    "[{}] ERROR: test: [{}] could not be killed!",
                    shared::get_date_and_time(),
                    task_id
                );
                response.success = false;
                response.error = Some("Could not stop test");
            }
        },
        None => {
            response.message = "Task does not exist. Nothing to stop";
        }
    }
    return Ok(serde_json::to_string(&response).unwrap());
}

pub async fn delete_test(
    project_id: &str,
    script_id: &str,
    test_id: &str,
    running_tests: Data<&Arc<RwLock<HashMap<String, task::Task>>>>,
    /*red_client: Data<&redis::Client>,*/
) -> Result<String, Box<dyn Error>> {
    let mut response = models::http::Response::<String> {
        success: true,
        message: "Test delete",
        error: None,
        content: None,
    };
    let task_id = shared::encode_test_id(&project_id, &script_id, &test_id);
    if stop_test(&task_id, &running_tests /*red_client*/)
        .await
        .is_err()
    {
        response.success = false;
        response.error = Some("Could not stop test");
        return Ok(serde_json::to_string(&response).unwrap());
    }
    if shared::delete_test(&project_id, &script_id, &test_id).is_err() {
        response.success = false;
        response.error = Some("Could not delete test");
    }
    return Ok(serde_json::to_string(&response).unwrap());
}

pub async fn stop_prefix(
    prefix: &str,
    running_tests: Data<&Arc<RwLock<HashMap<String, task::Task>>>>,
) -> Result<String, Box<dyn Error>> {
    let mut response = models::http::Response::<HashMap<String, bool>> {
        success: true,
        message: "Prefix stop",
        error: None,
        content: None,
    };
    let mut error = String::new();
    let mut stopped_tests = HashMap::new();
    for running_test in running_tests.write().iter_mut() {
        if running_test.0.starts_with(prefix) {
            match running_test.1.kill() {
                Ok(_) => {
                    println!(
                        "[{}] TEST KILLED: [{}]!",
                        shared::get_date_and_time(),
                        running_test.0
                    );
                    stopped_tests.insert(running_test.0.to_owned(), true);
                }
                Err(_) => {
                    eprintln!(
                        "[{}] ERROR: test: [{}] could not be killed!",
                        shared::get_date_and_time(),
                        running_test.0
                    );
                    error.push_str(&format!(
                        "test: [{}] could not be killed!\n",
                        running_test.0
                    ));
                    response.success = false;
                    stopped_tests.insert(running_test.0.to_owned(), false);
                }
            }
        }
    }
    if !response.success {
        response.error = Some(&error);
    }
    response.content = Some(stopped_tests);
    return Ok(serde_json::to_string(&response).unwrap());
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
    running_tests: Data<&Arc<RwLock<HashMap<String, task::Task>>>>,
) -> Result<String, Box<dyn Error>> {
    let mut response = shared::models::http::Response {
        success: true,
        message: "Tests",
        error: None,
        content: None,
    };

    let mut content = shared::models::http::tests::Content {
        tests: Vec::new(),
        config: shared::get_config(&project_id, &script_id),
    };
    let script_dir =
        match std::fs::read_dir(shared::get_a_script_results_dir(project_id, script_id)) {
            Ok(dir) => dir,
            Err(_) => {
                response.error = Some("Could not read results directory");
                response.content = Some(content);
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
        if running_tests.read().contains_key(&task_id) {
            status = 0;
        }
        //get info
        let info = shared::get_info(project_id, script_id, &test_id);
        //get history
        let history = None;
        //let history = shared::get_results_history(project_id, script_id, &test_id);
        content.tests.push(shared::models::Test {
            id: test_id,
            project_id: project_id.to_owned(),
            script_id: script_id.to_owned(),
            status,
            results,
            history,
            info,
        });
    }
    response.content = Some(content);
    let response = serde_json::to_string(&response).unwrap();
    Ok(response)
}

pub async fn all_running_tests(running_tests: Data<&Arc<RwLock<HashMap<String, task::Task>>>>) -> Result<String, Box<dyn Error>> {
    let mut response = shared::models::http::Response {
        success: true,
        message: "Tests",
        error: None,
        content: None,
    };

    let mut content = shared::models::http::tests::Content {
        tests: Vec::new(),
        config: None,
    };
    for running_test in running_tests.read().keys() {
        let (project_id, script_id, test_id) = shared::decode_test_id(running_test);
        //get results
        let results = shared::get_results(&project_id, &script_id, &test_id);
        let status = 0;
        //get info
        let info = shared::get_info(&project_id, &script_id, &test_id);
        //get history
        let history = None;
        //let history = shared::get_results_history(&project_id, &script_id, &test_id);
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

pub async fn delete_projects(
    projects_to_be_deleted: Json<models::http::projects::ProjectIds>,
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
    
    for project_id in projects_to_be_deleted.project_ids.iter() {
        
        //TODO stop project and delete it and notify
        // let mut stop_project_error = String::new();
        // let stop_response = stop_project(&project_id, &workers, &mut stop_project_error).await;
        // if stop_response.success {
        //     //delete project if all tests are stopped
        //     let mut delete_project_error = String::new();
        //     let delete_response = delete_project(&project_id, &mut delete_project_error);
        //     if delete_response.success {
        //         contents.insert(
        //             project_id.to_owned(),
        //             (true, delete_project_error.to_owned()),
        //         );
        //         //notify browser
        //         let websocket_message = models::websocket::WebSocketMessage {
        //             event_type: shared::PROJECT_DELETED,
        //             event: models::websocket::projects::DeletedProject {
        //                 id: project_id.to_owned(),
        //             },
        //         };
        //         if main_sender
        //             .send(serde_json::to_string(&websocket_message).unwrap())
        //             .is_err()
        //         {
        //             println!(
        //                 "[{}] DELETE PROJECT EVENT: No clients are connected!",
        //                 shared::get_date_and_time()
        //             );
        //         }
        //     } else {
        //         response.success = false;
        //         contents.insert(
        //             project_id.to_owned(),
        //             (false, delete_project_error.to_owned()),
        //         );
        //     }
        // } else {
        //     response.success = false;
        //     contents.insert(
        //         project_id.to_owned(),
        //         (false, stop_project_error.to_owned()),
        //     );
        // }
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