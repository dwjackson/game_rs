use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::process::Command;

const EXIT_SUCCESS: i32 = 0;

#[derive(Debug)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub dir: Option<String>,
    pub command: Vec<String>,
    pub env: HashMap<String, String>,
    pub tags: Vec<String>,
    pub installed: bool,
}

impl Game {
    pub fn format(&self) -> String {
        format!("{} - {}", self.id, self.name)
    }

    pub fn run<'a>(&'a self) -> Result<(), GameError<'a>> {
        if !self.installed {
            return Err(GameError::NotInstalled);
        }

        if let Some(dir) = &self.dir {
            let path = Path::new(dir);
            if env::set_current_dir(path).is_err() {
                return Err(GameError::CouldNotChangeDirectory(dir));
            }
        }
        let mut command = Command::new(&self.command[0]);
        command.args(&self.command[1..]);
        for (k, v) in self.env.iter() {
            command.env(k, v);
        }
        match command.status() {
            Ok(status) => {
                if let Some(code) = status.code()
                    && code != EXIT_SUCCESS
                {
                    let cmd = format!("{:?}", command);
                    return Err(GameError::CommandReturnedFailure(cmd));
                }
            }
            Err(_) => {
                return Err(GameError::ExecutionFailed);
            }
        }

        Ok(())
    }

    pub fn is_installed(&self) -> bool {
        self.installed
    }
}

pub enum GameError<'a> {
    NoGameId,
    CouldNotChangeDirectory(&'a str),
    NoSuchGame(&'a str),
    CommandReturnedFailure(String),
    ExecutionFailed,
    NotInstalled,
    CouldNotWriteStats(String),
}
