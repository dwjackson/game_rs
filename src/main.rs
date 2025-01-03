use homedir::my_home;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use toml::{Table, Value};

const CONFIG_FILE_NAME: &str = "games.toml";

fn main() {
    let home_dir = my_home().expect("No home directory found").unwrap();
    let config_path = Path::new(&home_dir).join(CONFIG_FILE_NAME);
    let config_contents = fs::read_to_string(&config_path).expect("No games.toml config found");
    match parse_config(&config_contents) {
        Ok(games) => {
            let args: Vec<String> = env::args().collect();
            if args.len() < 2 {
                println!("USAGE: games [COMMAND]");
            } else {
                let cmd = &args[1];
                match cmd.as_str() {
                    "list" => {
                        command_list(&games);
                    }
                    "play" => command_play(&games, &args[2..]),
                    _ => println!("Unrecognized command: {}", cmd),
                }
            }
        }
        Err(e) => match e {
            ParseError::MissingName(id) => println!("Game missing name: {}", id),
            ParseError::MissingCommand(id) => println!("Game missing cmd: {}", id),
            ParseError::GameNotTable => println!("The 'game' key must correspond to a table"),
            ParseError::MissingGameTable => println!("A 'game' table is required'"),
        },
    }
}

fn command_list(games: &Games) {
    let mut game_ids: Vec<&String> = games.games.keys().collect();
    game_ids.sort();
    for game_id in game_ids.iter() {
        let game = games.find(game_id).unwrap();
        println!("{}", game.format());
    }
}

fn command_play(games: &Games, args: &[String]) {
    if args.is_empty() {
        panic!("A game_id is required");
    }
    let game_id = &args[0];
    match games.find(game_id) {
        Some(game) => {
            game.run();
        }
        None => panic!("No such game: {}", game_id),
    }
}

struct Game {
    id: String,
    name: String,
    dir: Option<String>,
    command: String,
    env: HashMap<String, String>,
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
        let command_parts: Vec<&str> = self.command.split_whitespace().collect();
        let mut command = Command::new(command_parts[0]);
        command.args(&command_parts[1..]);
        for (k, v) in self.env.iter() {
            command.env(k, v);
        }
        command.status().expect("Failed to execute game");
    }
}

struct Games {
    games: HashMap<String, Game>,
}

impl Games {
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
                let command = if let Some(Value::String(scummvm_id)) = game_config.get("scummvm_id")
                {
                    format!("scummvm {}", scummvm_id)
                } else if let Some(Value::String(wine_exe)) = game_config.get("wine_exe") {
                    format!("mangohud wine {}", wine_exe)
                } else if let Some(Value::String(dosbox_conf_file)) =
                    game_config.get("dosbox_config")
                {
                    format!("dosbox -conf {}", dosbox_conf_file)
                } else {
                    match game_config.get("cmd") {
                        Some(Value::String(cmd)) => cmd.to_string(),
                        _ => return Err(ParseError::MissingCommand(game_id.clone())),
                    }
                };
                let dir_prefix = game_config.get_str("dir_prefix");
                let dir_prefix = directories.get_str(dir_prefix);
                let dir = game_config.get_str("dir");
                let env = match game_config.get("env") {
                    Some(Value::Table(tbl)) => {
                        let mut environment = HashMap::new();
                        for (k, v) in tbl.iter() {
                            if let Value::String(s) = v {
                                environment.insert(k.clone(), s.as_str().to_string());
                            }
                        }
                        environment
                    }
                    _ => HashMap::new(),
                };
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
                    env,
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
        assert!(games.find("morrowind").is_some());
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
        assert!(games.find("atlantis").is_some());
    }
}
