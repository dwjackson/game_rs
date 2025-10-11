use crate::Game;
use crate::ParseError;
use crate::Settings;
use std::collections::HashMap;
use std::path::Path;
use toml::{Table, Value};

pub struct GameBuilder<'a> {
    id: String,
    directories: &'a Table,
    settings: &'a Settings,
    name: Option<String>,
    dir: String,
    dir_prefix: String,
    command: Vec<String>,
    env: HashMap<String, String>,
    tags: Vec<String>,
    use_mangohud: Option<bool>,
    fps_limit: Option<i64>,
    use_gamescope: bool,
    use_vk: bool,
    installed: bool,
}

impl<'a> GameBuilder<'a> {
    pub fn new(id: String, directories: &'a Table, settings: &'a Settings) -> GameBuilder<'a> {
        GameBuilder {
            id,
            directories,
            settings,
            name: None,
            dir: "".to_string(),
            dir_prefix: "".to_string(),
            command: Vec::new(),
            env: HashMap::new(),
            tags: Vec::new(),
            use_mangohud: None,
            fps_limit: None,
            use_gamescope: false,
            use_vk: true,
            installed: true,
        }
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn command(mut self, command: Vec<String>) -> Self {
        self.command = command;
        self
    }

    pub fn dir_prefix(mut self, dir_prefix: String) -> Self {
        self.dir_prefix = dir_prefix;
        self
    }

    pub fn dir(mut self, dir: String) -> Self {
        self.dir = dir;
        self
    }

    pub fn env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn mangohud(mut self, use_mangohud: bool) -> Self {
        self.use_mangohud = Some(use_mangohud);
        self
    }

    pub fn is_wine(&self) -> bool {
        !self.command.is_empty() && self.command[0] == "wine"
    }

    pub fn fps_limit(mut self, limit: i64) -> Self {
        self.fps_limit = Some(limit);
        self
    }

    pub fn use_gamescope(mut self) -> Self {
        self.use_gamescope = true;
        self
    }

    pub fn use_vk(mut self, b: bool) -> Self {
        self.use_vk = b;
        self
    }

    pub fn not_installed(mut self) -> Self {
        self.installed = false;
        self
    }

    pub fn build(self) -> Result<Game, ParseError> {
        if self.name.is_none() {
            return Err(ParseError::MissingName(self.id.clone()));
        }
        if self.command.is_empty() {
            return Err(ParseError::MissingCommand(self.id.clone()));
        }

        let is_wine = self.is_wine();

        let dir_prefix = if !self.dir_prefix.is_empty() {
            match self.directories.get(&self.dir_prefix) {
                Some(Value::String(s)) => s.to_string(),
                _ => {
                    return Err(ParseError::NoSuchDirectoryPrefix(
                        self.id.clone(),
                        self.dir_prefix.clone(),
                    ));
                }
            }
        } else {
            self.dir_prefix
        };

        let dir = match self.directories.get(&self.dir) {
            Some(Value::String(d)) => d.to_string(),
            _ => self.dir,
        };

        let game_dir = Path::new(&dir_prefix)
            .join(&dir)
            .to_str()
            .unwrap()
            .to_string();

        let use_mangohud = self.use_mangohud.is_some() && self.use_mangohud.unwrap()
            || self.use_mangohud.is_none() && is_wine;

        let command = if self.settings.use_gamescope {
            let cmd = format!(
                "gamescope -W {} -H {} -f --force-grab-cursor",
                self.settings.width, self.settings.height
            );
            let mut c = shell_words::split(&cmd).expect("Failed to split gamescope command");
            if let Some(i) = self.fps_limit {
                c.push("-r".to_string());
                c.push(i.to_string());
            }
            if use_mangohud {
                c.push("--mangoapp".to_string());
            }
            c.push("--".to_string());
            for x in self.command.into_iter() {
                c.push(x);
            }
            c
        } else if use_mangohud {
            let mut c = Vec::new();
            c.push("mangohud".to_string());
            for x in self.command.into_iter() {
                c.push(x);
            }
            c
        } else {
            self.command
        };

        let mut env = self.env;
        if use_mangohud && let Some(limit) = self.fps_limit {
            env.insert(
                "MANGOHUD_CONFIG".to_string(),
                format!("fps_limit={}", limit),
            );
        }

        if !self.use_vk {
            env.insert(
                "WINEDLLOVERRIDES".to_string(),
                "*d3d9,*d3d10,*d3d10_1,*d3d10core,*d3d11,*dxgi=b".to_string(),
            );
        }

        Ok(Game {
            id: self.id,
            name: self.name.unwrap(),
            command,
            dir: if !game_dir.is_empty() {
                Some(game_dir)
            } else {
                None
            },
            env,
            tags: self.tags,
            installed: self.installed,
        })
    }
}
