use crate::models;
use parking_lot::RwLock;
use poem::web::{Data, Json};
use redis::Commands;
use std::error::Error;
use std::fs::canonicalize;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::sleep;

pub async fn start_test(
    project_id: &str,
    script_id: &str,
    mut req: Json<models::http::TestInfo>,
    running_tests: Data<&Arc<RwLock<HashMap<String, Child>>>>,
    currently_running_tests: Data<&Arc<AtomicBool>>,
    red_client: Data<&redis::Client>,
    ip: Data<&String>,
) -> Result<String, Box<dyn Error>> {
    //let workers = req.workers.unwrap_or(1);
    let mut response = models::http::Response {
        success: true,
        message: "Test start",
        error: None,
        content: None,
    };
    let mut red_connection = red_client.get_connection().unwrap();
    //check if project is locked
    let locked_projects: std::collections::HashSet<String> =
        if let Ok(set) = red_connection.smembers(shared::LOCKED_PROJECTS) {
            set
        } else {
            HashSet::new()
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

    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros()
        .to_string();

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
    //let workers_command = format!("--workers {}", workers);

    //lock before running
    let _: () = red_connection
        .sadd(shared::LOCKED_PROJECTS, &project_id)
        .unwrap();

    let mut running_tests_guard = running_tests.write();
    //checking if the script was not deleted in the meantime after performing the lock
    if !locust_file.exists() {
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
        Command::new(Path::new(&env_dir).join("Scripts").join("locust.exe"))
            .current_dir(shared::get_a_project_dir(&project_id))
            .args(&args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?
    } else {
        let can_locust_location_linux =
            canonicalize(Path::new(&env_dir).join("bin").join("locust")).unwrap();
        Command::new("bash")
            .current_dir(shared::get_a_project_dir(&project_id))
            .args(&[
                "-c",
                &format!(
                    "{} -f {} --headless {} {} {} {} {} {}",
                    can_locust_location_linux.to_str().ok_or("Run Error")?,
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
    };

    let task_id = shared::encode_test_id(&project_id, &script_id, &id);
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
        .unwrap();

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
        .unwrap();
    //unlock //TODO! what happens on error?
    let _: () = red_connection
        .srem(shared::LOCKED_PROJECTS, &project_id)
        .unwrap();
    //run the garbage collector
    if !currently_running_tests.load(Ordering::SeqCst) {
        currently_running_tests.store(true, Ordering::SeqCst); //TODO! hmm
        println!(
            "[{}] SCRIPTS GARBAGE COLLECTOR: Running!",
            shared::get_date_and_time()
        );
        let tokio_currently_running_tests = currently_running_tests.clone();
        let tokio_running_tests = Arc::clone(&running_tests);
        tokio::spawn(async move {
            loop {
                let mut tests_info_map: HashMap<String, Vec<models::websocket::tests::TestInfo>> =
                    HashMap::new();
                {
                    let mut tokio_tests_guard = tokio_running_tests.write();
                    if tokio_tests_guard.len() < 1 {
                        tokio_currently_running_tests.store(false, Ordering::SeqCst);
                        println!(
                            "[{}] SCRIPTS GARBAGE COLLECTOR: Terminating!",
                            shared::get_date_and_time()
                        );
                        break;
                    }
                    let mut to_be_removed: Vec<String> = Vec::new();
                    //collect info if a user is connected
                    let wanted_scripts: HashSet<String> =
                        if let Ok(set) = red_connection.smembers(shared::SUBS) {
                            set
                        } else {
                            HashSet::new()
                        };
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
                                    }
                                    None => {
                                        println!("[{}] SCRIPTS GARBAGE COLLECTOR: Script [{}] terminated by signal!",shared::get_date_and_time(), id);
                                    }
                                }
                                //remove from redis
                                let _: () =
                                    red_connection.srem(shared::RUNNING_TESTS, &id).unwrap();
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
                    let _: () = red_connection
                        .publish(
                            "main_channel",
                            serde_json::to_string(&redis_message).unwrap(),
                        )
                        .unwrap();

                    // let _: () = red_connection
                    //     .set(
                    //         script_id,
                    //         serde_json::to_string(&websocket_message).unwrap(),
                    //     )
                    //     .unwrap();
                    // let _: () = red_connection.expire(script_id, 5).unwrap();
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
    response.content = Some(started_test);
    return Ok(serde_json::to_string(&response).unwrap());
}

pub async fn stop_test(
    task_id: &str,
    running_tests: &Data<&Arc<RwLock<HashMap<String, Child>>>>,
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
    running_tests: Data<&Arc<RwLock<HashMap<String, Child>>>>,
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

pub fn register(red_connection: &mut redis::Connection,
    worker_ip: &str,
){
    loop {
        if let Ok(()) = red_connection.sadd(shared::REGISTERED_WORKERS, &worker_ip) {
            println!(
                "[{}] WORKER: Registered!",
                shared::get_date_and_time(),
            );
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
}

pub async fn remove_all_running_tests(
    red_connection: &mut redis::Connection,
    worker_ip: &str,
) -> Result<(), Box<dyn Error>> {
    let running_tests: std::collections::HashSet<String>;
    loop {
        if let Ok(set) = red_connection.smembers(shared::RUNNING_TESTS) {
            running_tests = set;
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(3));
    }

    for test_id in running_tests {
        //get tests worked ip
        let (project_id, script_id, test_id_d) = shared::decode_test_id(&test_id);
        let test_worker_ip =
            shared::get_worker_ip(project_id, script_id, test_id_d).ok_or("Id Error")?;
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
            loop {
                if let Ok(()) = red_connection.publish(
                    "main_channel",
                    serde_json::to_string(&redis_message).unwrap(),
                ) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
            //remove from redis
            loop {
                if let Ok(()) = red_connection.srem(shared::RUNNING_TESTS, &test_id) {
                    println!(
                        "[{}] OLD RUNNING TEST REMOVED!: [{}] ",
                        shared::get_date_and_time(),
                        test_id
                    );
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
        }
    }
    Ok(())
}

pub async fn stop_prefix(
    prefix: &str,
    running_tests: Data<&Arc<RwLock<HashMap<String, Child>>>>,
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
