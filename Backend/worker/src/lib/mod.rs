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
    let mut running_tests_guard = running_tests.write();
    //checking if the script was not deleted in the meantime after performing the lock
    if !locust_file.exists() {
        return Ok(String::from("Script was deleted!"));
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
        Command::new("/usr/bin/bash")
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
    let mut red_connection = red_client.get_connection().unwrap();
    let _: () = red_connection
        .sadd(shared::RUNNING_TESTS, &task_id)
        .unwrap();

    running_tests_guard.insert(task_id, cmd);

    //run the garbage collector
    if !currently_running_tests.load(Ordering::SeqCst) {
        let tokio_currently_running_tests = currently_running_tests.clone();
        let tokio_running_tests = Arc::clone(&running_tests);
        tokio::spawn(async move {
            loop {
                {
                    let mut tokio_tests_guard = tokio_running_tests.write();
                    if tokio_tests_guard.len() < 1 {
                        tokio_currently_running_tests.store(false, Ordering::SeqCst);
                        println!("SCRIPTS GARBAGE COLLECTOR: Terminating!");
                        break;
                    }
                    println!("SCRIPTS GARBAGE COLLECTOR: Running!");
                    let mut to_be_removed: Vec<String> = Vec::new();
                    //collect info if a user is connected
                    let wanted_scripts: HashSet<String> = if let Ok(set) = red_connection.smembers(shared::SUBS){
                        set
                    } else {
                        HashSet::new()
                    };
                    for (id, cmd) in tokio_tests_guard.iter_mut() {
                        let (project_id, script_id, test_id) = shared::decode_test_id(id);
                        let global_script_id = shared::get_global_script_id(id);
                        let mut test = models::Test {
                            id: id.to_owned(),
                            script_id: script_id.to_owned(),
                            project_id: project_id.to_owned(),
                            status: Some(0),
                            results: None,
                            info: None,
                        };
                        //check if the script is wanted and save results in redis
                        if wanted_scripts.contains(global_script_id){
                            println!("SCRIPT WANTED: {}", global_script_id);
                            // get info and send through redis channel
                        }
                        
                        match cmd.try_wait() {
                            Ok(Some(exit_status)) => {
                                // process finished
                                test.status = Some(1); // process finished
                                to_be_removed.push(id.to_owned());
                                match exit_status.code() {
                                    Some(code) => {
                                        println!("SCRIPTS GARBAGE COLLECTOR: Script [{}] terminated with code [{}]!", id, code);
                                    }
                                    None => {
                                        println!("SCRIPTS GARBAGE COLLECTOR: Script [{}] terminated by signal!", id);
                                    }
                                }
                                //remove from redis
                                let _: () =
                                    red_connection.srem(shared::RUNNING_TESTS, &id).unwrap();
                            }
                            Ok(None) => {
                                test.status = Some(0); // process is running
                            }
                            Err(e) => {
                                println!("ERROR: SCRIPTS GARBAGE COLLECTOR: Script [{}]: could not wait on child process error: {:?}", id, e);
                            }
                        }
                    }
                    //remove finished
                    for id in to_be_removed.iter() {
                        tokio_tests_guard.remove_entry(id);
                        println!("SCRIPTS GARBAGE COLLECTOR: Script [{}] removed!", id);
                    }
                }

                sleep(Duration::from_secs(3)).await;
            }
        });
        currently_running_tests.store(true, Ordering::SeqCst);
    } else {
        println!("SCRIPTS GARBAGE COLLECTOR: Already running!");
    }
    // Notify channel

    let started_test = shared::models::Test {
        id: id,
        project_id: project_id.to_string(),
        script_id: script_id.to_string(),
        status: Some(0),
        results: None,
        info: Some(test_info),
    };
    response.content = Some(started_test);
    return Ok(serde_json::to_string(&response).unwrap());
}

pub async fn stop_test(
    project_id: &str,
    script_id: &str,
    test_id: &str,
    running_tests: Data<&Arc<RwLock<HashMap<String, Child>>>>,
    red_client: Data<&redis::Client>,
) -> Result<String, Box<dyn Error>> {
    let mut response = models::http::Response::<String> {
        success: true,
        message: "Test stop",
        error: None,
        content: None,
    };
    let task_id = shared::encode_test_id(project_id, script_id, test_id);
    let mut running_tests_guard = running_tests.write();
    match running_tests_guard.get_mut(&task_id) {
        Some(cmd) => match cmd.kill() {
            Ok(_) => {
                println!("TEST KILLED: [{}]!", task_id);
                response.message = "Task stopped";
                running_tests_guard.remove_entry(&task_id);
                //remove from redis
                let mut red_connection = red_client.get_connection().unwrap();
                let _: () =
                red_connection.srem(shared::RUNNING_TESTS, &task_id).unwrap();
            }
            Err(_) => {
                println!("ERROR: test: [{}] could not be killed!", task_id);
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
