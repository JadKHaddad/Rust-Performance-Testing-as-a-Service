use std::process::{Child, ExitStatus};

pub enum Task {
    NormalTask(Child),
    MasterTask(Child, Vec<Child>),
}

impl Task {
    pub fn kill(&mut self) -> std::io::Result<()>{
        match self {
            Task::NormalTask(child) => child.kill(),
            Task::MasterTask(master, children) => {
                for child in children {
                    if child.kill().is_err() {
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Could not kill child process"));
                    }
                }
                master.kill()
            }
        }
    }
    pub fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        match self {
            Task::NormalTask(child) => child.try_wait(),
            Task::MasterTask(master, _) => {
                master.try_wait()
            }
        }
    }
}