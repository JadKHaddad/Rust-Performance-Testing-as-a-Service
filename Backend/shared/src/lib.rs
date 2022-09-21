use chrono::{DateTime, NaiveDateTime, Utc};
use csv::Reader;
use port_scanner::local_port_available;
use redis::cmd;
use redis::RedisResult;
use redis::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio::time::{sleep, Duration};

pub const DATA_DIR: &str = "Performance-Testing-Data";
//pub const DOWNLOADS_DIR: &str = "downloads";
pub const PROJECTS_DIR: &str = "projects";
pub const TEMP_DIR: &str = "temp";
pub const ENVIRONMENTS_DIR: &str = "environments";
pub const RESULTS_DIR: &str = "results";
//redis subscriptions
pub const SUBS: &str = "SUBS";
//redis running tests
pub const RUNNING_TESTS: &str = "RUNNING_TESTS";
//redis locked projects
pub const LOCKED_PROJECTS: &str = "LOCKED_PROJECTS";
//redis registered workers
pub const REGISTERED_WORKERS: &str = "REGISTERED_WORKERS";

//events
pub const INFORMATION: &str = "INFORMATION";
pub const UPDATE_TEST_INFO: &str = "UPDATE";
pub const TEST_STARTED: &str = "TEST_STARTED";
pub const TEST_STOPPED: &str = "TEST_STOPPED";
pub const TEST_DELETED: &str = "TEST_DELETED";
pub const PROJECT_DELETED: &str = "PROJECT_DELETED";

pub mod models;
pub mod plot;
pub mod zip;

pub fn get_a_free_port() -> Result<u16, String> {
    let mut port = 5000;
    loop {
        port += 1;
        if local_port_available(port) {
            return Ok(port);
        }
        if port > 50000 {
            return Err("No free port found!".to_owned());
        }
    }
}

pub fn get_date_and_time<'a>() -> chrono::format::DelayedFormat<chrono::format::StrftimeItems<'a>> {
    let now: DateTime<Utc> = Utc::now();
    now.format("%Y.%m.%d %H:%M:%S")
}

pub fn get_data_dir() -> PathBuf {
    Path::new("..").join(DATA_DIR)
}

pub fn get_temp_dir() -> PathBuf {
    get_data_dir().join(TEMP_DIR)
}

pub fn get_environments_dir() -> PathBuf {
    get_data_dir().join(ENVIRONMENTS_DIR)
}

// pub fn get_downloads_dir() -> PathBuf {
//     get_data_dir().join(DOWNLOADS_DIR)
// }

// pub fn get_results_dir() -> PathBuf {
//     get_data_dir().join(RESULTS_DIR)
// }

pub fn get_projects_dir() -> PathBuf {
    get_data_dir().join(PROJECTS_DIR)
}

pub fn get_a_project_dir(id: &str) -> PathBuf {
    get_projects_dir().join(id)
}

pub fn get_a_temp_dir(id: &str) -> PathBuf {
    get_temp_dir().join(id)
}

pub fn get_an_environment_dir(id: &str) -> PathBuf {
    get_environments_dir().join(id)
}

pub fn get_a_locust_dir(id: &str) -> PathBuf {
    get_a_project_dir(id).join("locust")
}

pub fn get_a_project_results_dir(id: &str) -> PathBuf {
    get_a_project_dir(id).join(RESULTS_DIR)
}

pub fn get_a_script_results_dir(project_id: &str, script_id: &str) -> PathBuf {
    get_a_project_results_dir(project_id).join(script_id)
}

pub fn get_config_file(project_id: &str, script_id: &str) -> PathBuf {
    get_a_locust_dir(project_id).join(format!("{}.json", script_id))
}

pub fn get_a_test_results_dir(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    get_a_script_results_dir(project_id, script_id).join(test_id)
}

pub fn get_zip_file(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    get_a_script_results_dir(project_id, script_id)
        .join(test_id)
        .join("results.zip")
}

pub fn get_plot_file(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    get_a_script_results_dir(project_id, script_id)
        .join(test_id)
        .join("results.png")
}

pub fn encode_script_id(project_id: &str, script_id: &str) -> String {
    format!("{}]$[{}", project_id, script_id)
}

pub fn get_global_script_id(test_id: &str) -> &str {
    let index = test_id.rfind("]$[").unwrap();
    &test_id[0..index]
}

pub fn encode_test_id(project_id: &str, script_id: &str, test_id: &str) -> String {
    format!("{}]$[{}]$[{}", project_id, script_id, test_id)
}

pub fn decode_test_id(test_id: &str) -> (&str, &str, &str) {
    let mut parts = test_id.split("]$[");
    let project_id = parts.next().unwrap();
    let script_id = parts.next().unwrap();
    let test_id = parts.next().unwrap();
    (project_id, script_id, test_id)
}

pub fn get_log_file_relative_path(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    Path::new("../..")
        .join(PROJECTS_DIR)
        .join(project_id)
        .join(RESULTS_DIR)
        .join(script_id)
        .join(test_id)
        .join("log.log")
}

pub fn get_log_file_relative_path_for_worker(
    project_id: &str,
    script_id: &str,
    test_id: &str,
    worker_id: u32,
) -> PathBuf {
    Path::new("../..")
        .join(PROJECTS_DIR)
        .join(project_id)
        .join(RESULTS_DIR)
        .join(script_id)
        .join(test_id)
        .join(&format!("worker_{}_log.log", worker_id))
}

pub fn get_csv_file_path(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    get_a_test_results_dir(project_id, script_id, test_id).join("results_stats.csv")
}

pub fn get_csv_history_file_path(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    get_a_test_results_dir(project_id, script_id, test_id).join("results_stats_history.csv")
}

pub fn get_csv_file_relative_path(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    Path::new("../..")
        .join(PROJECTS_DIR)
        .join(project_id)
        .join(RESULTS_DIR)
        .join(script_id)
        .join(test_id)
        .join("results")
}

pub fn get_info_file_path(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    get_a_test_results_dir(project_id, script_id, test_id).join("info.json")
}

pub fn get_results(
    project_id: &str,
    script_id: &str,
    test_id: &str,
) -> Option<Vec<models::ResultRow>> {
    let csv_file = get_csv_file_path(project_id, script_id, &test_id);
    let mut rdr = match Reader::from_path(csv_file) {
        Ok(rdr) => rdr,
        Err(_) => return None,
    };
    let mut results = Vec::new();
    for result in rdr.deserialize() {
        let row: models::ResultRow = match result {
            Ok(record) => record,
            Err(_) => return None,
        };
        results.push(row);
    }
    return Some(results);
}

pub fn get_config(project_id: &str, script_id: &str) -> Option<models::TestConfig> {
    let config_file = get_config_file(project_id, script_id);
    println!("{:?}", config_file);
    let json_string = match std::fs::read_to_string(config_file) {
        Ok(res) => res,
        Err(_) => return None,
    };
    if let Ok(config) = serde_json::from_str(&json_string) {
        return Some(config);
    } else {
        return None;
    };
}

pub fn get_info(
    project_id: &str,
    script_id: &str,
    test_id: &str,
) -> Option<models::http::TestInfo> {
    let info_file = get_info_file_path(project_id, script_id, &test_id);
    let json_string = match std::fs::read_to_string(info_file) {
        Ok(res) => res,
        Err(_) => return None,
    };
    if let Ok(info) = serde_json::from_str(&json_string) {
        return Some(info);
    } else {
        return None;
    };
}

pub fn get_worker_ip(project_id: &str, script_id: &str, test_id: &str) -> Option<String> {
    if let Some(info) = get_info(project_id, script_id, test_id) {
        return info.worker_ip;
    } else {
        return None;
    }
}

pub fn get_results_history(
    project_id: &str,
    script_id: &str,
    test_id: &str,
) -> Option<Vec<models::ResultHistory>> {
    let csv_file = get_csv_history_file_path(project_id, script_id, &test_id);
    let mut rdr = match Reader::from_path(csv_file) {
        Ok(rdr) => rdr,
        Err(_) => return None,
    };
    let mut results = Vec::new();
    for result in rdr.deserialize() {
        let row: models::ResultHistory = match result {
            Ok(record) => record,
            Err(_) => return None,
        };
        results.push(row);
    }
    return Some(results);
}

pub fn get_parsed_results_history(
    project_id: &str,
    script_id: &str,
    test_id: &str,
) -> Option<Vec<models::ParsedResultHistory>> {
    let csv_file = get_csv_history_file_path(project_id, script_id, &test_id);
    let mut rdr = match Reader::from_path(csv_file) {
        Ok(rdr) => rdr,
        Err(_) => return None,
    };
    let mut results = Vec::new();
    for result in rdr.deserialize() {
        let row: models::ResultHistory = match result {
            Ok(record) => record,
            Err(_) => return None,
        };

        if let Ok(parsed_timestamp) = row.timestamp.parse::<i64>() {
            let naive = NaiveDateTime::from_timestamp(parsed_timestamp, 0);
            let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);

            let total_median_response_time = row
                .total_median_response_time
                .parse::<f32>()
                .unwrap_or_default();
            let total_average_response_time = row
                .total_average_response_time
                .parse::<f32>()
                .unwrap_or_default();
            let total_min_response_time = row
                .total_min_response_time
                .parse::<f32>()
                .unwrap_or_default();
            let total_max_response_time = row
                .total_max_response_time
                .parse::<f32>()
                .unwrap_or_default();

            results.push(models::ParsedResultHistory {
                datetime: datetime,
                total_median_response_time: total_median_response_time,
                total_average_response_time: total_average_response_time,
                total_min_response_time: total_min_response_time,
                total_max_response_time: total_max_response_time,
            });
        } else {
            return None;
        }
    }
    return Some(results);
}

pub fn get_last_result_history(
    project_id: &str,
    script_id: &str,
    test_id: &str,
) -> Option<models::ResultHistory> {
    let csv_file = get_csv_history_file_path(project_id, script_id, &test_id);
    let mut rdr = match Reader::from_path(csv_file) {
        Ok(rdr) => rdr,
        Err(_) => return None,
    };
    match rdr.deserialize().last() {
        Some(result) => {
            let row: models::ResultHistory = match result {
                Ok(record) => record,
                Err(_) => return None,
            };
            return Some(row);
        }
        None => return None,
    }
}

pub fn delete_test(
    project_id: &str,
    script_id: &str,
    test_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    //get test folder and delete it
    let test_dir = get_a_test_results_dir(&project_id, &script_id, &test_id);

    //sometimes on windows the folder is not deleted but info is deleted so lets back it up
    let info_file = get_info_file_path(&project_id, &script_id, &test_id);
    let test_info = std::fs::read_to_string(&info_file)?;

    // zip is contained
    // let zip_file = get_zip_file(&project_id, &script_id, &test_id);
    // std::fs::remove_file(&zip_file)?;

    match std::fs::remove_dir_all(&test_dir) {
        Ok(_) => {
            println!(
                "[{}] TEST DELETED: [{}]!",
                get_date_and_time(),
                test_dir.to_str().ok_or("System Error")?
            );
        }
        Err(e) => {
            eprintln!(
                "[{}] ERROR: test: [{}] could not be deleted! Error: {:?}",
                get_date_and_time(),
                test_dir.to_str().ok_or("System Error")?,
                e
            );
            let mut file = std::fs::File::create(&test_dir.join("info.json"))?;
            file.write(test_info.as_bytes())?;
        }
    }
    return Ok(());
}

#[derive(Clone)]
pub struct Manager {
    client: redis::Client,
    connection: Arc<Mutex<redis::Connection>>,
    reconnecting: Arc<Mutex<bool>>,
    tx: tokio::sync::broadcast::Sender<bool>,
}

/// The Manager acts as connector to the redis server. The Creation of the
/// Manager is blocking until a connection is established.
/// The Manager will automatically - non-blocking - reconnect on all query
/// failures. Be sure to use a valid query. Tokio compatible.
impl Manager {
    pub async fn new(client: redis::Client) -> Manager {
        let connection;
        loop {
            if let Ok(con) = client.get_connection() {
                connection = con;
                println!("[{}] REDIS MANAGER: Connected!", get_date_and_time());
                break;
            }
            println!("[{}] REDIS MANAGER: Reconnecting!", get_date_and_time());
            sleep(Duration::from_secs(3)).await;
        }
        let (tx, _) = broadcast::channel::<bool>(100);
        Manager {
            connection: Arc::new(Mutex::new(connection)),
            client: client,
            reconnecting: Arc::new(Mutex::new(false)),
            tx: tx,
        }
    }

    fn reconnect(&mut self) {
        let mut reconnecting = self.reconnecting.lock().unwrap();
        if *reconnecting {
            return;
        }
        *reconnecting = true;
        let connection = self.connection.clone();
        let client = self.client.clone();
        let reconnecting = self.reconnecting.clone();
        let mut rx = self.tx.subscribe();
        tokio::spawn(async move {
            loop {
                if rx.try_recv().is_ok() {
                    println!(
                        "[{}] REDIS MANAGER: Reconnection thread terminated!",
                        get_date_and_time()
                    );
                    break;
                }
                println!("[{}] REDIS MANAGER: Reconnecting!", get_date_and_time());
                if let Ok(mut x) = connection.lock() {
                    if let Ok(connection) = client.get_connection() {
                        *x = connection;
                        println!("[{}] REDIS MANAGER: Reconnected!", get_date_and_time());
                        break;
                    }
                }
                sleep(Duration::from_secs(2)).await;
            }
            let mut reconnecting = reconnecting.lock().unwrap();
            *reconnecting = false;
        });
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        if self.tx.send(true).is_ok() {
            println!(
                "[{}] REDIS MANAGER: Terminaiting Reconnection thread!",
                get_date_and_time()
            );
        }
    }
}

impl redis::ConnectionLike for Manager {
    fn get_db(&self) -> i64 {
        self.client.get_connection_info().redis.db
    }

    fn req_packed_command(&mut self, cmd: &[u8]) -> RedisResult<Value> {
        let connection = self.connection.clone();
        let mut x = connection.lock().unwrap();
        let result = x.req_packed_command(cmd);
        if result.is_err() {
            self.reconnect();
        }
        result
    }

    fn req_packed_commands<'a>(
        &'a mut self,
        cmd: &[u8],
        offset: usize,
        count: usize,
    ) -> RedisResult<Vec<Value>> {
        let connection = self.connection.clone();
        let mut x = connection.lock().unwrap();
        let result = x.req_packed_commands(cmd, offset, count);
        if result.is_err() {
            self.reconnect();
        }
        result
    }

    fn is_open(&self) -> bool {
        let x = self.connection.lock().unwrap();
        x.is_open()
    }

    fn check_connection(&mut self) -> bool {
        cmd("PING").query::<String>(self).is_ok()
    }
}
