use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::process::Command;
use toml::{Table, Value};
use homedir::my_home;
use std::fs;

const CONFIG_FILE_NAME: &str = "games.toml";

fn main() {
    let home_dir = my_home().expect("No home directory found").unwrap();
    let config_path = Path::new(&home_dir).join(CONFIG_FILE_NAME);
    let config_contents = fs::read_to_string(&config_path).expect("No games.toml config found");
    match parse_config(&config_contents) {
        Ok(games) => (),
        Err(e) => panic!("{:?}", e),
    }
}

struct Game {
    id: String,
    name: String,
    dir: Option<String>,
    command: String,
}

impl Game {
    fn format(&self) -> String {
        format!("{} - {}", self.id, self.name)
    }

    fn run(&self) {
        if let Some(dir) = &self.dir {
            let path = Path::new(dir);
            if let Err(e) = env::set_current_dir(path) {
                panic!("Could not change directory: {:?}", e);
            }
        }
        Command::new(&self.command)
            .status()
            .expect("Failed to execute game");
    }
}

struct Games {
    games: HashMap<String, Game>,
}

impl Games {
    fn exists(&self, id: &str) -> bool {
        self.games.contains_key(id)
    }

    fn find(&self, id: &str) -> Option<&Game> {
        self.games.get(id)
    }
}

trait GetStr {
    fn get_str(&self, key: &str) -> &str;
}

impl GetStr for Table {
    fn get_str(&self, key: &str) -> &str {
        match self.get(key) {
            Some(Value::String(s)) => s,
            _ => "",
        }
    }
}

fn parse_config(config_content: &str) -> Result<Games, ParseError> {
    let mut games = HashMap::new();
    let config = config_content.parse::<Table>().unwrap();
    let directories = match config.get("directories") {
        Some(Value::Table(tbl)) => tbl,
        _ => &Table::new(),
    };
    if let Value::Table(games_config) = &config["games"] {
        for (game_id, value) in games_config.iter() {
            if let Value::Table(game_config) = &value {
                let name = if let Value::String(game_name) = &game_config["name"] {
                    game_name.to_string()
                } else {
                    return Err(ParseError::MissingName(game_id.clone()));
                };
                let command = if let Some(Value::String(scummvm_id)) = game_config.get("scummvm_id") {
                    format!("scummvm {}", scummvm_id)
                } else if let Some(Value::String(wine_exe)) = game_config.get("wine_exe") {
                    format!("mangohud wine {}", wine_exe)
                } else {
                    match game_config.get("cmd") {
                        Some(Value::String(cmd)) => cmd.to_string(),
                        _ => return Err(ParseError::MissingCommand(game_id.clone())),
                    }
                };
                let dir_prefix = game_config.get_str("dir_prefix");
                let dir_prefix = directories.get_str(dir_prefix);
                let dir = game_config.get_str("dir");
                let game_dir = Path::new(dir_prefix)
                    .join(dir)
                    .to_str()
                    .unwrap()
                    .to_string();
                let game = Game {
                    id: game_id.clone(),
                    name,
                    dir: if !game_dir.is_empty() {
                        Some(game_dir)
                    } else {
                        None
                    },
                    command,
                };
                games.insert(game_id.clone(), game);
            } else {
                return Err(ParseError::GameNotTable);
            }
        }
    } else {
        return Err(ParseError::MissingGameTable);
    }
    Ok(Games { games })
}

#[derive(Debug)]
enum ParseError {
    MissingName(String),
    MissingCommand(String),
    GameNotTable,
    MissingGameTable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_exists() {
        let config = "[games]\n[games.morrowind]\nname = \"Morrowind\"\ncmd = \"openmw\"";
        let games = parse_config(config).expect("Bad config");
        assert!(games.exists("morrowind"));
    }

    #[test]
    fn test_format_game() {
        let config = "[games]\n[games.morrowind]\nname = \"Morrowind\"\ncmd = \"openmw\"";
        let games = parse_config(config).expect("Bad config");
        if let Some(game) = games.find("morrowind") {
            let s = game.format();
            assert_eq!(s, "morrowind - Morrowind");
        } else {
            panic!("Game not found");
        }
    }

    #[test]
    fn test_parse_game() {
        let config = "[games]\n[games.morrowind]\nname = \"Morrowind\"\ncmd = \"openmw\"";
        let games = parse_config(config).expect("Bad config");
        if let Some(game) = games.find("morrowind") {
            assert_eq!(game.command, "openmw");
        } else {
            panic!("Game not found");
        }
    }

    #[test]
    fn test_parse_game_with_directory() {
        let config = "[games]\n[games.quake]\nname = \"Quake\"\ndir = \"/home/test/Games/quake\"\ncmd=\"vkquake\"";
        let games = parse_config(config).expect("Bad config");
        if let Some(game) = games.find("quake") {
            assert_eq!(game.dir.as_ref().unwrap(), "/home/test/Games/quake");
        } else {
            panic!("Game not found");
        }
    }

    #[test]
    fn test_game_with_directory_prefix() {
        let config = "
        [directories]
        games_dir=\"/home/test/Games\"

        [games]
        
        [games.quake]
        name = \"Quake\"
        dir_prefix=\"games_dir\"
        dir = \"quake\"
        cmd=\"vkquake\"
        ";
        let games = parse_config(config).expect("Bad config");
        if let Some(game) = games.find("quake") {
            assert_eq!(game.dir.as_ref().unwrap(), "/home/test/Games/quake");
        } else {
            panic!("Game not found");
        }
    }

    #[test]
    fn test_scummvm_game() {
        let config = "[games]\n[games.atlantis]\nname = \"Indiana Jones and the Fate of Atlantis\"\nscummvm_id = \"atlantis\"";
        let games = parse_config(config).expect("Bad config");
        assert!(games.exists("atlantis"));
    }
}
