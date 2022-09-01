use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize)]
pub struct Test {
    pub id: String,
    pub script_id: String,
    pub project_id: String,
    pub status: u8, // 0 running, 1 finished
    pub results: Option<Vec<ResultRow>>,
    pub history: Option<Vec<ResultHistory>>,
    pub info: Option<http::TestInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ResultRow {
    #[serde(rename(serialize = "type"))]
    #[serde(rename(deserialize = "Type"))]
    r#type: String,
    #[serde(rename(serialize = "name"))]
    #[serde(rename(deserialize = "Name"))]
    name: String,
    #[serde(rename(serialize = "request_count"))]
    #[serde(rename(deserialize = "Request Count"))]
    request_count: String,
    #[serde(rename(serialize = "failure_count"))]
    #[serde(rename(deserialize = "Failure Count"))]
    failure_count: String,
    #[serde(rename(serialize = "median_response_time"))]
    #[serde(rename(deserialize = "Median Response Time"))]
    median_response_time: String,
    #[serde(rename(serialize = "avarage_response_time"))]
    #[serde(rename(deserialize = "Average Response Time"))]
    avarage_response_time: String,
    #[serde(rename(serialize = "min_response_time"))]
    #[serde(rename(deserialize = "Min Response Time"))]
    min_response_time: String,
    #[serde(rename(serialize = "max_response_time"))]
    #[serde(rename(deserialize = "Max Response Time"))]
    max_response_time: String,
    #[serde(rename(serialize = "avarage_content_size"))]
    #[serde(rename(deserialize = "Average Content Size"))]
    avarage_content_size: String,
    #[serde(rename(serialize = "requests_per_second"))]
    #[serde(rename(deserialize = "Requests/s"))]
    requests_per_second: String,
    #[serde(rename(serialize = "failures_per_seconde"))]
    #[serde(rename(deserialize = "Failures/s"))]
    failures_per_second: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ResultHistory {
    #[serde(rename(serialize = "timestamp"))]
    #[serde(rename(deserialize = "Timestamp"))]
    pub timestamp: String,
    #[serde(rename(serialize = "total_median_response_time"))]
    #[serde(rename(deserialize = "Total Median Response Time"))]
    pub total_median_response_time: String,
    #[serde(rename(serialize = "total_average_response_time"))]
    #[serde(rename(deserialize = "Total Average Response Time"))]
    pub total_average_response_time: String,
    #[serde(rename(serialize = "total_min_response_time"))]
    #[serde(rename(deserialize = "Total Min Response Time"))]
    pub total_min_response_time: String,
    #[serde(rename(serialize = "total_max_response_time"))]
    #[serde(rename(deserialize = "Total Max Response Time"))]
    pub total_max_response_time: String,
}
pub struct ParsedResultHistory {
    pub datetime: DateTime<Utc>,
    pub total_median_response_time: f32,
    pub total_average_response_time: f32,
    pub total_min_response_time: f32,
    pub total_max_response_time: f32,
}

pub mod redis {
    use serde::{Deserialize, Serialize};
    #[derive(Debug, Deserialize, Serialize)]
    pub struct RedisMessage {
        pub event_type: String,
        pub id: String,
        pub message: String,
    }
}

pub mod websocket {
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    pub struct WebSocketMessage<'a, T>
    where
        T: Serialize,
    {
        pub event_type: &'a str,
        pub event: T,
    }

    pub mod information {
        use serde::Serialize;

        #[derive(Debug, Serialize)]
        pub struct Event {
            pub connected_clients_count: u32,
            pub running_tests_count: u32,
            pub istalling_projects: Vec<String>,
        }
    }

    pub mod projects {
        use serde::Serialize;

        #[derive(Debug, Serialize)]
        pub struct Event {
            pub istalling_projects: Vec<Project>,
        }

        #[derive(Debug, Serialize)]
        pub struct Project {
            pub id: String,
            pub status: u8, // 0 running, 1 finished, 2 failed
            pub error: Option<String>,
        }

        #[derive(Debug, Serialize)]
        pub struct DeletedProject {
            pub id: String,
        }
    }

    pub mod tests {
        use serde::Serialize;

        #[derive(Debug, Serialize)]
        pub struct TestInfoEvent<'a> {
            pub tests_info: &'a Vec<TestInfo>,
        }

        #[derive(Debug, Serialize)]
        pub struct TestInfo {
            pub id: String,
            pub status: u8, // 0 running, 1 finished
            pub results: Option<Vec<super::super::ResultRow>>,
            pub last_history: Option<super::super::ResultHistory>,
        }

        #[derive(Debug, Serialize)]
        pub struct TestDeletedEvent {
            pub id: String,
        }

        #[derive(Debug, Serialize)]
        pub struct TestStoppeddEvent {
            pub id: String,
        }
    }
}

pub mod http {
    use serde::Deserialize;
    use serde::Serialize;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct WorkerInfo {
        pub worker_name: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TestInfo {
        pub project_id: Option<String>,
        pub script_id: Option<String>,
        pub users: Option<u32>,
        pub spawn_rate: Option<u32>,
        pub workers: Option<u32>,
        pub host: Option<String>,
        pub time: Option<u32>,
        pub description: Option<String>,
        pub id: Option<String>,
        pub worker_ip: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Script {
        pub project_id: String,
        pub script_id: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Test {
        pub test_id: String,
        pub project_id: String,
        pub script_id: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Response<'a, T>
    where
        T: Serialize,
    {
        pub success: bool,
        pub message: &'a str,
        pub error: Option<&'a str>,
        pub content: Option<T>,
    }

    #[derive(Debug, Serialize)]
    pub struct ErrorResponse<'a> {
        pub success: bool,
        pub message: &'a str,
        pub error: &'a str,
    }

    impl<'a> ErrorResponse<'a> {
        pub fn new(error: &'a str) -> Self {
            Self {
                success: false,
                message: "Server Error",
                error: error,
            }
        }
    }

    pub mod projects {
        use serde::Serialize;
        use serde::Deserialize;

        #[derive(Debug, Serialize, Deserialize)]
        pub struct ProjectIds{
            pub project_ids: Vec<String>,
        }

        #[derive(Debug, Serialize)]
        pub struct Content {
            pub projects: Vec<Project>,
        }

        #[derive(Debug, Serialize)]
        pub struct Project {
            pub id: String,
            pub scripts: Vec<String>,
        }
    }

    pub mod scripts {
        use serde::Serialize;

        #[derive(Debug, Serialize)]
        pub struct Content {
            pub scripts: Vec<String>,
        }
    }
    pub mod tests {
        use serde::Serialize;

        #[derive(Debug, Serialize)]
        pub struct Content {
            pub tests: Vec<super::super::Test>,
        }
    }
}
