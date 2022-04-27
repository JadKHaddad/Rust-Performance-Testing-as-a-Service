use crate::models;
use parking_lot::RwLock;
use poem::web::{Data, Json};
use std::error::Error;
use std::fs::canonicalize;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    collections::HashMap,
    path::{Path,},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::sleep;

pub async fn start_test(
    mut req: Json<models::http::TestParameter>,
    running_tests: Data<&Arc<RwLock<HashMap<String, Child>>>>,
    currently_running_tests: Data<&Arc<AtomicBool>>,
    ip: Data<&String>,
) -> Result<String, Box<dyn Error>> {
    println!("{:?}", req);
    let project_id = &req.project_id;
    let script_id = &req.script_id;
    //let workers = req.workers.unwrap_or(1);

    let locust_file = shared::get_a_locust_dir(project_id).join(script_id);

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
    let test_dir = shared::get_a_test_results_dir(project_id, script_id, &id);
    std::fs::create_dir_all(&test_dir)?;

    //define paths
    let env_dir = shared::get_an_environment_dir(&project_id);
    let can_project_dir = canonicalize(shared::get_a_project_dir(&project_id)).unwrap(); //absolute path for commands current dir
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
            .current_dir(shared::get_a_project_dir(&project_id))
            .args(&["-c"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?
    };

    let test_id = shared::encode_test_id(&project_id, &script_id, &id);
    //save test info
    req.id = Some(id.clone());
    let mut file = std::fs::File::create(&test_dir.join("info.json"))?;
    file.write(serde_json::to_string(&*req).unwrap().as_bytes())?;
    let mut file = std::fs::File::create(&test_dir.join("ip"))?;
    file.write(ip.as_bytes())?;
    running_tests_guard.insert(test_id, cmd);

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
                    for (id, cmd) in tokio_tests_guard.iter_mut() {
                        let (project_id, script_id, test_id) = shared::decode_test_id(id);
                        let mut test = models::Test {
                            id: id.to_owned(),
                            script_id: script_id.to_owned(),
                            project_id: project_id.to_owned(),
                            status: Some(0),
                            results: None,
                        };
                        //check if the script is wanted and save results in redis
                        match cmd.try_wait() {
                            Ok(Some(exit_status)) => {
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
    // save id in redis
    Ok(String::from("allright"))
}

pub async fn stop_test(
    mut req: Json<models::http::Test>,
    running_tests: Data<&Arc<RwLock<HashMap<String, Child>>>>,
) -> Result<String, Box<dyn Error>> { 
    Ok(String::from("yes"))
}


