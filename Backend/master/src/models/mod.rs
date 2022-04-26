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
    }
}

pub mod http {
    use serde::Deserialize;
    use serde::Serialize;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct TestParameter {
        pub project_id: String,
        pub script_id: String,
        pub users: Option<u32>,
        pub spawn_rate: Option<u32>,
        pub workers: Option<u32>,
        pub host: Option<String>,
        pub time: Option<u32>,
        pub description: Option<String>,
        pub id: Option<String>,
    }

    #[derive(Debug, Serialize)]
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
}
