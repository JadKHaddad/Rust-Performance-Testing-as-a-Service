pub mod websocket {
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    pub struct WebSocketMessage<'a, T> {
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
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    pub struct Response<'a, T> {
        pub success: bool,
        pub message: Option<&'a str>,
        pub content: Option<T>,
    }

    pub mod projects {
        use serde::Serialize;

        #[derive(Debug, Serialize)]
        pub struct Event<'a> {
            pub projects: Vec<Project<'a>>,
        }

        #[derive(Debug, Serialize)]
        pub struct Project<'a> {
            pub id: &'a str,
            pub scripts: Vec<&'a str>,
        }
    }
}
