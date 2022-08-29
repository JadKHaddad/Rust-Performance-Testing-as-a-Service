use std::process::{Child, ExitStatus};

pub enum Task {
    NormalTask(Child, String),
    MasterTask(Child, Vec<Child>, String),
}

impl Task {
    pub fn kill(&mut self) -> std::io::Result<()>{
        match self {
            Task::NormalTask(child, _) => child.kill(),
            Task::MasterTask(master, children, _) => {
                for child in children {
                    //ok => stopped, err => was not running
                    child.kill().unwrap_or_default();
                }
                master.kill()
            }
        }
    }
    pub fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        match self {
            Task::NormalTask(child, _) => child.try_wait(),
            Task::MasterTask(master, _, _) => {
                master.try_wait()
            }
        }
    }
}

//implement drop trait for task
impl Drop for Task {
    fn drop(&mut self) {
        /*match self {
            Task::NormalTask(child, _) => {
                child.kill().unwrap_or_default();
            }
            Task::MasterTask(master, children, _) => {
                for child in children {
                    child.kill().unwrap_or_default();
                }
                master.kill().unwrap_or_default();
            }
        }*/
        let id;
        match self {
            Task::NormalTask(_, id_) => {
                id = std::mem::take(id_);
            }
            Task::MasterTask(_, _, id_) => {
                id = std::mem::take(id_);
            }
        }
        println!("[{}] TASK [{}] dropped!", shared::get_date_and_time(), id);
    }
}