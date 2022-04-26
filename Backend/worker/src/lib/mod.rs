use crate::models;
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

pub async fn start_test(
    mut req: Json<models::http::TestParameter>,
    running_tests: Data<&Arc<RwLock<HashMap<String, Child>>>>,
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

    let test_id = shared::get_test_id(&project_id, &script_id, &id);
    //save test info as json
    req.id = Some(id.clone());
    let mut file = std::fs::File::create(&test_dir.join("info.json"))?;
    file.write(serde_json::to_string(&*req).unwrap().as_bytes())?;

    running_tests_guard.insert(test_id, cmd);

    //run the garbage collector
    // save id in redis
    Ok(String::from("allright"))
}
