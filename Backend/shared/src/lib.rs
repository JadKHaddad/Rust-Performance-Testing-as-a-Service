use std::path::{Path, PathBuf};

pub const DATA_DIR: &str = "data";
pub const DOWNLOADS_DIR: &str = "downloads";
pub const PROJECTS_DIR: &str = "projects";
pub const TEMP_DIR: &str = "temp";
pub const ENVIRONMENTS_DIR: &str = "environments";
pub const RESULTS_DIR: &str = "results";

pub mod models;

pub fn get_data_dir() -> PathBuf {
    Path::new("..").join(DATA_DIR)
}

pub fn get_temp_dir() -> PathBuf {
    get_data_dir().join(TEMP_DIR)
}

pub fn get_downloads_dir() -> PathBuf {
    get_data_dir().join(DOWNLOADS_DIR)
}

pub fn get_projects_dir() -> PathBuf {
    get_data_dir().join(PROJECTS_DIR)
}

pub fn get_environments_dir() -> PathBuf {
    get_data_dir().join(ENVIRONMENTS_DIR)
}

pub fn get_results_dir() -> PathBuf {
    get_data_dir().join(RESULTS_DIR)
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
    get_results_dir().join(id)
}

pub fn get_a_script_results_dir(project_id: &str, script_id: &str) -> PathBuf {
    get_a_project_results_dir(project_id).join(script_id)
}

pub fn get_a_test_results_dir(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    get_a_script_results_dir(project_id, script_id).join(test_id)
}

pub fn encode_test_id(project_id: &str, script_id: &str, test_id: &str) -> String {
    format!("{}]$[{}]$[{}", project_id, script_id, test_id)
}

pub fn decode_test_id(test_id: &str) -> (&str, &str, &str) {
    let mut parts = test_id.split("]$[");
    let project_id = parts.next().unwrap();
    let script_id = parts.next().unwrap();
    let test_id = parts.next().unwrap();
    (
        project_id,
        script_id,
        test_id,
    )
}

pub fn get_log_file_relative_path(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    Path::new("../..")
        .join(RESULTS_DIR)
        .join(project_id)
        .join(script_id)
        .join(test_id)
        .join("log.log")
}

pub fn get_csv_file_path(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    get_a_test_results_dir(project_id, script_id, test_id).join("results_stats.csv")
}

pub fn get_csv_file_relative_path(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    Path::new("../..")
        .join(RESULTS_DIR)
        .join(project_id)
        .join(script_id)
        .join(test_id)
        .join("results")
}

pub fn get_info_file_path(project_id: &str, script_id: &str, test_id: &str) -> PathBuf {
    get_a_test_results_dir(project_id, script_id, test_id).join("info.json")
}

pub fn get_results(project_id: &str, script_id: &str, test_id: &str) -> Option<String> {
    let csv_file = get_csv_file_path(project_id, script_id, &test_id);
    let results = match std::fs::read_to_string(csv_file) {
        Ok(res) => Some(res),
        Err(_) => None,
    };
    return results;
}

pub fn get_info(project_id: &str, script_id: &str, test_id: &str) -> Option<models::http::TestInfo> {
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

pub fn get_worker_ip(project_id: &str, script_id: &str, test_id: &str) -> String {
    let ip_path = get_a_test_results_dir(project_id, script_id, test_id).join("ip");
    //open file and read to string and return
    String::from("ip")
}