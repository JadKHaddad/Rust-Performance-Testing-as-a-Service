use crate::models;
pub mod task;
use parking_lot::RwLock;
use poem::web::{Data, Json};
use redis::Commands;
use std::error::Error;
use std::fs::canonicalize;
use std::io::Write;

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;

pub async fn start_test(
    project_id: &str,
    script_id: &str,
    mut req: Json<models::http::TestInfo>,
    running_tests: Data<&Arc<RwLock<HashMap<String, task::Task>>>>,
    currently_running_tests: Data<&Arc<Mutex<bool>>>,
    red_client: redis::Client,
    red_manager: Data<&shared::manager::Manager>,
    ip: Data<&String>,
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
    let red_client = red_client.clone();
    let mut red_connection;
    if let Ok(connection) = red_client.get_connection() {
        red_connection = connection;
    } else {
        response.error = Some("Could not connect to database");
        response.success = false;
        return Ok(serde_json::to_string(&response).unwrap());
    }
    //check if project is locked
    let locked_projects: std::collections::HashSet<String>;
    if let Ok(set) = red_connection.smembers(shared::LOCKED_PROJECTS) {
        locked_projects = set;
    } else {
        response.error = Some("Could not connect to database");
        response.success = false;
        return Ok(serde_json::to_string(&response).unwrap());
    };

    if locked_projects.contains(project_id) {
        //TODO! run in scheduler
        response.error = Some("Project is currently locked");
        response.success = false;
        return Ok(serde_json::to_string(&response).unwrap());
    }

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

    //lock before running
    if red_connection
        .sadd::<_, _, ()>(shared::LOCKED_PROJECTS, &project_id)
        .is_err()
    {
        //delete test dir
        std::fs::remove_dir_all(&test_dir)?;
        response.error = Some("Could not lock project");
        response.success = false;
        return Ok(serde_json::to_string(&response).unwrap());
    }

    let mut running_tests_guard = running_tests.write();
    //checking if the script was not deleted in the meantime after performing the lock
    if !locust_file.exists() {
        //unlock
        let _: () = red_connection
            .srem(shared::LOCKED_PROJECTS, &project_id)
            .unwrap_or_default();
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
                //unlock
                let _: () = red_connection
                    .srem(shared::LOCKED_PROJECTS, &project_id)
                    .unwrap_or_default();
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
                        .stdout(Stdio::inherit())
                        .stderr(Stdio::inherit())
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
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()?,
                children,
                task_id.clone(),
            )
        } else {
            task::Task::NormalTask(
                Command::new(Path::new(&env_dir).join("Scripts").join("locust.exe"))
                    .current_dir(shared::get_a_project_dir(&project_id))
                    .args(&args)
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
                //unlock
                let _: () = red_connection
                    .srem(shared::LOCKED_PROJECTS, &project_id)
                    .unwrap_or_default();
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
                    format!("--worker-id={}", i+1)
                }else{
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
                        .stdout(Stdio::inherit())
                        .stderr(Stdio::inherit())
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
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()?,
                children,
                task_id.clone(),
            )
        } else {
            task::Task::NormalTask(
                Command::new("bash")
                    .current_dir(shared::get_a_project_dir(&project_id))
                    .args(&["-c", &command])
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
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
        worker_ip: Some(ip.to_string()),
    };
    let mut file = std::fs::File::create(&test_dir.join("info.json"))?;
    file.write(serde_json::to_string(&test_info).unwrap().as_bytes())?;

    // save id in redis
    let _: () = red_connection
        .sadd(shared::RUNNING_TESTS, &task_id)
        .unwrap_or_default();

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
    let redis_message = models::redis::RedisMessage {
        event_type: websocket_message.event_type.to_owned(),
        id: shared::encode_script_id(&project_id, &script_id),
        message: serde_json::to_string(&websocket_message).unwrap(),
    };
    let _: () = red_connection
        .publish(
            "main_channel",
            serde_json::to_string(&redis_message).unwrap(),
        )
        .unwrap_or_default();

    //unlock //TODO! what happens on error?
    let _: () = red_connection
        .srem(shared::LOCKED_PROJECTS, &project_id)
        .unwrap_or_default();
    
    //run the garbage collector
    if let Ok(mut currently_running_tests_mutex) = currently_running_tests.lock()
    {
        if !*currently_running_tests_mutex {
            *currently_running_tests_mutex = true;
            println!(
                "[{}] SCRIPTS GARBAGE COLLECTOR: Running!",
                shared::get_date_and_time()
            );
            let tokio_currently_running_tests = currently_running_tests.clone();
            let tokio_running_tests = Arc::clone(&running_tests);
            let mut red_manager = red_manager.clone();
            tokio::spawn(async move {
                loop {
                    let mut tests_info_map: HashMap<String, Vec<models::websocket::tests::TestInfo>> =
                        HashMap::new();
                    {
                        let mut tokio_tests_guard = tokio_running_tests.write();
                        if tokio_tests_guard.len() < 1 {
                            if let Ok(mut lock) = tokio_currently_running_tests.lock(){
                                *lock = false;
                                println!(
                                    "[{}] SCRIPTS GARBAGE COLLECTOR: Terminating!",
                                    shared::get_date_and_time()
                                );
                            }else{
                                eprintln!("[{}] ERROR: SCRIPTS GARBAGE COLLECTOR: failed to lock", shared::get_date_and_time());
                            }
                            break;
                        }
                        let mut to_be_removed: Vec<String> = Vec::new();
                        //collect info if a user is connected
                        let mut wanted_scripts: HashSet<String> = HashSet::new();

                        if let Ok(set) = red_manager.smembers(shared::SUBS) {
                            wanted_scripts = set;
                        }
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
                                            println!("[{}] SCRIPTS GARBAGE COLLECTOR: Script [{}] terminated by signal!",shared::get_date_and_time(), id);
                                        }
                                    }
                                    //remove from redis
                                    if let Ok(mut connection) = red_client.get_connection() {
                                        let _: () = connection
                                            .srem(shared::RUNNING_TESTS, &id)
                                            .unwrap_or_default();
                                    }
                                }
                                Ok(None) => {
                                    status = 0; // process is running
                                }
                                Err(e) => {
                                    eprintln!("[{}] ERROR: SCRIPTS GARBAGE COLLECTOR: Script [{}]: could not wait on child process error: {:?}",shared::get_date_and_time(), id, e);
                                }
                            }
                            //check if the script is wanted and save results in redis
                            if wanted_scripts.contains(global_script_id) {
                                println!(
                                    "[{}] SCRIPT WANTED: {}",
                                    shared::get_date_and_time(),
                                    global_script_id
                                );
                                let results = shared::get_results(project_id, script_id, test_id);
                                let last_history =
                                    shared::get_last_result_history(project_id, script_id, test_id);
                                let test_info = models::websocket::tests::TestInfo {
                                    id: test_id.to_owned(),
                                    results: results,
                                    status: status,
                                    last_history: last_history,
                                };
                                if tests_info_map.contains_key(global_script_id) {
                                    tests_info_map
                                        .get_mut(global_script_id)
                                        .unwrap()
                                        .push(test_info);
                                } else {
                                    tests_info_map.insert(global_script_id.to_owned(), vec![test_info]);
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
                            event: models::websocket::tests::TestInfoEvent {
                                tests_info: tests_info,
                            },
                        };
                        let redis_message = models::redis::RedisMessage {
                            event_type: websocket_message.event_type.to_owned(),
                            id: script_id.to_owned(),
                            message: serde_json::to_string(&websocket_message).unwrap(),
                        };
                        let _: () = red_manager
                            .publish(
                                "main_channel",
                                serde_json::to_string(&redis_message).unwrap(),
                            )
                            .unwrap_or_default();
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
    }else{
        eprintln!("[{}] ERROR: WORKER: Test start failed to lock", shared::get_date_and_time());
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

pub fn register(red_client: &redis::Client, worker_ip: &str) {
    println!(
        "[{}] WORKER: Registering worker",
        shared::get_date_and_time()
    );
    loop {
        if let Ok(mut connection) = red_client.get_connection() {
            if let Ok(()) = connection.sadd(shared::REGISTERED_WORKERS, &worker_ip) {
                println!("[{}] WORKER: Registered!", shared::get_date_and_time(),);
                break;
            }
        }
        eprintln!(
            "[{}] WORKER: Could not connect to redis. Trying again in 3 seconds.",
            shared::get_date_and_time()
        );
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
}

pub async fn remove_all_running_tests(
    red_client: &redis::Client,
    worker_ip: &str,
) -> Result<(), Box<dyn Error>> {
    loop {
        if let Ok(mut connection) = red_client.get_connection() {
            if let Ok(set) = connection.smembers(shared::RUNNING_TESTS) {
                let running_tests: std::collections::HashSet<String> = set;
                let mut success = true;
                for test_id in running_tests {
                    //get tests worked ip
                    let (project_id, script_id, test_id_d) = shared::decode_test_id(&test_id);
                    let test_worker_ip = shared::get_worker_ip(project_id, script_id, test_id_d)
                        .ok_or("Id Error")?;
                    if test_worker_ip == worker_ip {
                        //notify master
                        let websocket_message = models::websocket::WebSocketMessage {
                            event_type: shared::TEST_STOPPED,
                            event: models::websocket::tests::TestStoppeddEvent {
                                id: test_id_d.to_owned(),
                            },
                        };
                        let redis_message = models::redis::RedisMessage {
                            event_type: websocket_message.event_type.to_owned(),
                            id: shared::encode_script_id(project_id, script_id),
                            message: serde_json::to_string(&websocket_message).unwrap(),
                        };
                        println!(
                            "[{}] SENDING REDIS MESSAGE: {:?}",
                            shared::get_date_and_time(),
                            redis_message
                        );
                        //notify and remove from redis
                        if connection
                            .publish::<_, _, bool>(
                                "main_channel",
                                serde_json::to_string(&redis_message).unwrap(),
                            )
                            .is_err()
                            || connection
                                .srem::<_, _, bool>(shared::RUNNING_TESTS, &test_id)
                                .is_err()
                        {
                            success = false;
                            break;
                        }
                        println!(
                            "[{}] OLD RUNNING TEST REMOVED!: [{}] ",
                            shared::get_date_and_time(),
                            test_id
                        );
                    }
                }
                if success {
                    break;
                }
            }
        }
        eprintln!(
            "[{}] WORKER: Could not connect to redis. Trying again in 3 seconds.",
            shared::get_date_and_time()
        );
        std::thread::sleep(std::time::Duration::from_secs(3));
    }

    Ok(())
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
