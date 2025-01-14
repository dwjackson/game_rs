use homedir::my_home;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use toml::{Table, Value};

const CONFIG_FILE_NAME: &str = "games.toml";

type CommandHandler = for<'a> fn(games: &Games, args: &'a [String]) -> Result<(), GameError<'a>>;

fn main() {
    let config_contents = read_config();
    match parse_config(&config_contents) {
        Ok(games) => {
            let args: Vec<String> = env::args().collect();
            let commands = initialize_commands();

            if args.len() < 2 {
                println!("USAGE: game [COMMAND]");
                std::process::exit(1);
            }
            let cmd = args[1].as_str();
            if !commands.contains_key(cmd) {
                println!("Unrecognized command: {}", cmd);
                std::process::exit(1);
            }
            if let Err(e) = commands[cmd](&games, &args[2..]) {
                match e {
                    GameError::NoGameId => println!("A game ID is required"),
                    GameError::NoSuchGame(game_id) => println!("No such game: {}", game_id),
                }
                std::process::exit(1);
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

fn read_config() -> String {
    let home_dir = my_home().expect("No home directory found").unwrap();
    let config_path = Path::new(&home_dir).join(CONFIG_FILE_NAME);
    fs::read_to_string(&config_path).expect("No games.toml config found")
}

fn initialize_commands() -> HashMap<&'static str, CommandHandler> {
    let mut commands: HashMap<&str, CommandHandler> = HashMap::new();
    commands.insert("list", command_list);
    commands.insert("play", command_play);
    commands
}

fn command_list<'a>(games: &Games, _args: &[String]) -> Result<(), GameError<'a>> {
    let mut game_ids: Vec<&String> = games.games.keys().collect();
    game_ids.sort();
    for game_id in game_ids.iter() {
        let game = games.find(game_id).unwrap();
        println!("{}", game.format());
    }
    Ok(())
}

fn command_play<'a>(games: &Games, args: &'a [String]) -> Result<(), GameError<'a>> {
    if args.is_empty() {
        return Err(GameError::NoGameId);
    }
    let game_id = &args[0];
    match games.find(game_id) {
        Some(game) => {
            game.run();
            Ok(())
        }
        None => Err(GameError::NoSuchGame(game_id)),
    }
}

enum GameError<'a> {
    NoGameId,
    NoSuchGame(&'a str),
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
                    format!("wine {}", wine_exe)
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
                let use_mangohud = match game_config.get("use_mangohud") {
                    Some(Value::Boolean(b)) => *b,
                    _ => command.starts_with("wine"),
                };
                let command = if use_mangohud {
                    let mut c = "mangohud".to_string();
                    c.push(' ');
                    c.push_str(&command);
                    c
                } else {
                    command
                };
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
        let game = games.find("atlantis").unwrap();
        assert_eq!(game.command, "scummvm atlantis");
    }

    #[test]
    fn test_wine_game() {
        let config = "[games]\n[games.bg3]\nname = \"Baldur's Gate 3\"\ndir_prefix = \"wine_gog_dir\"\ndir=\"Baldur's Gate 3\"\nwine_exe = \"bg3.exe\"";
        let games = parse_config(config).expect("Bad config");
        let game = games.find("bg3").unwrap();
        assert_eq!(game.command, "mangohud wine bg3.exe");
    }

    #[test]
    fn test_dosbox_game() {
        let config =
            "[games]\n[games.sc2k]\nname = \"SimCity 2000\"\ndosbox_config = \"sc2k.conf\"";
        let games = parse_config(config).expect("Bad config");
        let game = games.find("sc2k").unwrap();
        assert_eq!(game.command, "dosbox -conf sc2k.conf");
    }

    #[test]
    fn test_wine_game_without_mangohud() {
        let config = "
        [games]
        [games.bg3]
        name = \"Baldur's Gate 3\"
        dir_prefix = \"wine_gog_dir\"
        dir=\"Baldur's Gate 3\"
        wine_exe = \"bg3.exe\"
        use_mangohud = false
        ";
        let games = parse_config(config).expect("Bad config");
        let game = games.find("bg3").unwrap();
        assert_eq!(game.command, "wine bg3.exe");
    }
}
