use homedir::my_home;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use toml::{Table, Value};

const USAGE: &str = "USAGE: game [COMMAND]";
const CONFIG_FILE_NAME: &str = "games.toml";
const DEFAULT_WIDTH: u32 = 1280;
const DEFAULT_HEIGHT: u32 = 720;

type CommandHandler = for<'a> fn(games: &'a Games, args: &'a [String]) -> Result<(), GameError<'a>>;

struct Settings {
    width: u32,
    height: u32,
    use_gamescope: bool,
}

struct GameCommand {
    cmd: &'static str,
    args: Vec<&'static str>,
    exec: CommandHandler,
    desc: &'static str,
}

fn main() {
    let config_contents_result = read_config();
    if config_contents_result.is_err() {
        println!(
            "Error: No {} config file found in home directory",
            CONFIG_FILE_NAME
        );
        std::process::exit(1);
    }
    let config_contents = config_contents_result.unwrap();
    match parse_config(&config_contents) {
        Ok(games) => {
            let args: Vec<String> = env::args().collect();
            let commands = initialize_commands();

            if args.len() < 2 {
                println!("{}", USAGE);
                std::process::exit(1);
            }
            let cmd = args[1].as_str();
            if !commands.contains_key(cmd) {
                println!("Unrecognized command: {}", cmd);
                std::process::exit(1);
            }
            let command = &commands[cmd];
            let handle = command.exec;
            if let Err(e) = handle(&games, &args[2..]) {
                match e {
                    GameError::NoGameId => println!("A game ID is required"),
                    GameError::CouldNotChangeDirectory(dir) => {
                        println!("Could not change directory to: {}", dir)
                    }
                    GameError::NoSuchGame(game_id) => println!("No such game: {}", game_id),
                    GameError::CommandReturnedFailure(cmd) => println!("Command failed: {}", cmd),
                    GameError::ExecutionFailed => println!("Could not execute game"),
                }
                std::process::exit(1);
            }
        }
        Err(e) => match e {
            ParseError::MissingName(id) => println!("Game missing name: {}", id),
            ParseError::MissingCommand(id) => println!("Game missing cmd: {}", id),
            ParseError::GameNotTable => println!("The 'game' key must correspond to a table"),
            ParseError::MissingGameTable => println!("A 'game' table is required'"),
            ParseError::NoSuchDirectoryPrefix(game_id, prefix) => println!(
                "Game {} has nonexistent directory prefix: {}",
                game_id, prefix
            ),
        },
    }
}

fn read_config() -> std::io::Result<String> {
    let home_dir = my_home().expect("No home directory found").unwrap();
    let config_path = Path::new(&home_dir).join(CONFIG_FILE_NAME);
    fs::read_to_string(&config_path)
}

fn initialize_commands() -> HashMap<&'static str, GameCommand> {
    let cmds = vec![
        GameCommand {
            cmd: "help",
            args: Vec::new(),
            exec: command_help,
            desc: "Explain the commands",
        },
        GameCommand {
            cmd: "list",
            args: vec!["TAG?"],
            exec: command_list,
            desc: "List games in the format \"game_id - name\"",
        },
        GameCommand {
            cmd: "play",
            args: vec!["GAME_ID"],
            exec: command_play,
            desc: "Play a game, specified by its game ID",
        },
        GameCommand {
            cmd: "tags",
            args: Vec::new(),
            exec: command_tags,
            desc: "List all tags",
        },
    ];
    let mut commands: HashMap<&str, GameCommand> = HashMap::new();
    for c in cmds.into_iter() {
        commands.insert(c.cmd, c);
    }
    commands
}

fn command_help<'a>(_games: &Games, _args: &[String]) -> Result<(), GameError<'a>> {
    let commands_hash = initialize_commands();
    let mut commands: Vec<&GameCommand> = commands_hash.values().collect();
    commands.sort_by(|a, b| a.cmd.cmp(b.cmd));

    println!("{}", USAGE);
    println!();
    println!("Commands: ");
    for c in commands.iter() {
        let args_str = if c.args.is_empty() {
            String::new()
        } else {
            format!(" [{}]", c.args.join("|"))
        };
        println!("\t{}{} - {}", c.cmd, args_str, c.desc);
    }
    Ok(())
}

fn command_list<'a>(games: &Games, args: &[String]) -> Result<(), GameError<'a>> {
    let mut game_ids: Vec<&String> = games.games.keys().collect();
    game_ids.sort();

    if !args.is_empty() {
        let tag = &args[0];
        // List all games having the given tag
        for game_id in game_ids.iter() {
            let game = games.find(game_id).unwrap();
            if game.has_tag(tag) {
                println!("{}", game.format());
            }
        }
    } else {
        for game_id in game_ids.iter() {
            let game = games.find(game_id).unwrap();
            println!("{}", game.format());
        }
    }
    Ok(())
}

fn command_tags<'a>(games: &Games, _args: &[String]) -> Result<(), GameError<'a>> {
    let game_ids: Vec<&String> = games.games.keys().collect();
    let tags = game_ids
        .iter()
        .flat_map(|game_id| {
            let game = games.find(game_id).unwrap();
            game.tags.iter().cloned()
        })
        .collect::<HashSet<String>>();
    let mut tags = tags.into_iter().collect::<Vec<String>>();
    tags.sort();
    let tags = tags;
    for tag in tags.iter() {
        println!("{}", tag);
    }
    Ok(())
}

fn command_play<'a>(games: &'a Games, args: &'a [String]) -> Result<(), GameError<'a>> {
    if args.is_empty() {
        return Err(GameError::NoGameId);
    }
    let game_id = &args[0];
    match games.find(game_id) {
        Some(game) => game.run(),
        None => Err(GameError::NoSuchGame(game_id)),
    }
}

enum GameError<'a> {
    NoGameId,
    CouldNotChangeDirectory(&'a str),
    NoSuchGame(&'a str),
    CommandReturnedFailure(String),
    ExecutionFailed,
}

struct Game {
    id: String,
    name: String,
    dir: Option<String>,
    command: Vec<String>,
    env: HashMap<String, String>,
    tags: Vec<String>,
}

impl Game {
    fn format(&self) -> String {
        format!("{} - {}", self.id, self.name)
    }

    fn run(&self) -> Result<(), GameError> {
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
                if let Some(code) = status.code() {
                    if code == 1 {
                        let cmd = format!("{:?}", command);
                        return Err(GameError::CommandReturnedFailure(cmd));
                    }
                }
            }
            Err(_) => {
                return Err(GameError::ExecutionFailed);
            }
        }

        Ok(())
    }

    fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
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

    let settings = match config.get("settings") {
        Some(Value::Table(tbl)) => {
            let width = match tbl.get("width") {
                Some(Value::Integer(i)) => *i as u32,
                _ => DEFAULT_WIDTH,
            };
            let height = match tbl.get("height") {
                Some(Value::Integer(i)) => *i as u32,
                _ => DEFAULT_HEIGHT,
            };
            let use_gamescope = match tbl.get("use_gamescope") {
                Some(Value::Boolean(b)) => *b,
                _ => false,
            };
            Settings {
                width,
                height,
                use_gamescope,
            }
        }
        _ => Settings {
            height: 0,
            width: 0,
            use_gamescope: false,
        },
    };

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
                    vec!["scummvm".to_string(), scummvm_id.to_string()]
                } else if let Some(Value::String(wine_exe)) = game_config.get("wine_exe") {
                    let mut cmd_parts = Vec::new();
                    cmd_parts.push("wine".to_string());
                    cmd_parts.push(wine_exe.to_string());
                    if let Some(Value::String(wine_args_string)) = game_config.get("wine_exe_args")
                    {
                        let wine_args = shell_words::split(wine_args_string)
                            .expect("Failed to parse wine command");
                        for arg in wine_args.into_iter() {
                            cmd_parts.push(arg);
                        }
                    }
                    cmd_parts
                } else if let Some(Value::String(dosbox_conf_file)) =
                    game_config.get("dosbox_config")
                {
                    vec![
                        "dosbox".to_string(),
                        "-conf".to_string(),
                        dosbox_conf_file.to_string(),
                    ]
                } else {
                    match game_config.get("cmd") {
                        Some(Value::String(cmd)) => {
                            shell_words::split(cmd).expect("Failed to parse shell command")
                        }
                        _ => return Err(ParseError::MissingCommand(game_id.clone())),
                    }
                };
                let dir_prefix = game_config.get_str("dir_prefix");
                let dir_prefix = if !dir_prefix.is_empty() {
                    match directories.get(dir_prefix) {
                        Some(Value::String(s)) => s,
                        _ => {
                            return Err(ParseError::NoSuchDirectoryPrefix(
                                game_id.to_string(),
                                dir_prefix.to_string(),
                            ))
                        }
                    }
                } else {
                    ""
                };
                let dir = game_config.get_str("dir");
                let mut env = match game_config.get("env") {
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
                    _ => command[0] == ("wine"),
                };
                let fps_limit = match game_config.get("fps_limit") {
                    Some(Value::Integer(i)) => Some(i),
                    _ => None,
                };
                let command = if settings.use_gamescope {
                    let cmd = format!(
                        "gamescope -W {} -H {} -f --force-grab-cursor",
                        settings.width, settings.height
                    );
                    let mut c =
                        shell_words::split(&cmd).expect("Failed to split gamescope command");
                    if let Some(i) = fps_limit {
                        c.push("-r".to_string());
                        c.push(i.to_string());
                    }
                    if use_mangohud {
                        c.push("--mangoapp".to_string());
                    }
                    c.push("--".to_string());
                    for x in command.into_iter() {
                        c.push(x);
                    }
                    c
                } else if use_mangohud {
                    if let Some(i) = fps_limit {
                        let fps_limit_setting = format!("fps_limit={}", i);
                        env.insert("MANGOHUD_CONFIG".to_string(), fps_limit_setting);
                    }
                    let mut c = Vec::new();
                    c.push("mangohud".to_string());
                    for x in command.into_iter() {
                        c.push(x);
                    }
                    c
                } else {
                    command
                };
                let tags = if let Some(Value::Array(tags_array)) = game_config.get("tags") {
                    tags_array
                        .iter()
                        .filter_map(|x| match x {
                            Value::String(tag) => Some(tag.to_string()),
                            _ => None,
                        })
                        .collect()
                } else {
                    Vec::new()
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
                    tags,
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
    NoSuchDirectoryPrefix(String, String),
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
            assert_eq!(game.command, vec!["openmw"]);
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
        assert_eq!(game.command, vec!["scummvm", "atlantis"]);
    }

    #[test]
    fn test_wine_game() {
        let config = "[games]\n[games.bg3]\nname = \"Baldur's Gate 3\"\ndir=\"Baldur's Gate 3\"\nwine_exe = \"bg3.exe\"";
        let games = parse_config(config).expect("Bad config");
        let game = games.find("bg3").unwrap();
        assert_eq!(game.command, vec!["mangohud", "wine", "bg3.exe"]);
    }

    #[test]
    fn test_dosbox_game() {
        let config =
            "[games]\n[games.sc2k]\nname = \"SimCity 2000\"\ndosbox_config = \"sc2k.conf\"";
        let games = parse_config(config).expect("Bad config");
        let game = games.find("sc2k").unwrap();
        assert_eq!(game.command, vec!["dosbox", "-conf", "sc2k.conf"]);
    }

    #[test]
    fn test_wine_game_without_mangohud() {
        let config = "
        [games]
        [games.bg3]
        name = \"Baldur's Gate 3\"
        dir=\"Baldur's Gate 3\"
        wine_exe = \"bg3.exe\"
        use_mangohud = false
        ";
        let games = parse_config(config).expect("Bad config");
        let game = games.find("bg3").unwrap();
        assert_eq!(game.command, vec!["wine", "bg3.exe"]);
    }

    #[test]
    fn test_game_started_by_shell_script() {
        let config = "
        [games]
        [games.test]
        name = \"Test Game\"
        dir=\"Test Game\"
        cmd = \"sh start.sh\"";
        let games = parse_config(config).expect("Bad config");
        let game = games.find("test").unwrap();
        assert_eq!(game.command, vec!["sh", "start.sh"]);
    }

    #[test]
    fn test_game_started_by_shell_script_with_spaces() {
        let config = "
        [games]
        [games.test]
        name = \"Test Game\"
        dir=\"Test Game\"
        cmd = \"sh 'start the game.sh'\"";
        let games = parse_config(config).expect("Bad config");
        let game = games.find("test").unwrap();
        assert_eq!(game.command, vec!["sh", "start the game.sh"]);
    }

    #[test]
    fn test_tags() {
        let config = "
        [games]
        [games.doom]
        name = \"Doom\"
        dir=\"Doom\"
        cmd = \"dsda-doom -iwad DOOM.WAD\"
        tags = [\"classic\", \"fps\"]";
        let games = parse_config(config).expect("Bad config");
        let game = games.find("doom").unwrap();
        assert_eq!(game.tags, vec!["classic", "fps"]);
        assert!(game.has_tag("fps"));
        assert!(!game.has_tag("rpg"));
    }

    #[test]
    fn test_wine_game_with_arguments() {
        let config = "
        [games]
        [games.test]
        name = \"Test Game\"
        dir=\"Test Game\"
        use_mangohud = false
        wine_exe = \"Test Game.exe\"
        wine_exe_args = \"-opt1 param1 -opt2\"";
        let games = parse_config(config).expect("Bad config");
        let game = games.find("test").unwrap();
        assert_eq!(
            game.command,
            vec!["wine", "Test Game.exe", "-opt1", "param1", "-opt2"]
        );
    }

    #[test]
    fn test_wine_game_with_mangohud_fps_limit() {
        let config = "
        [games]
        [games.test]
        name = \"Test Game\"
        dir=\"Test Game\"
        fps_limit = 60
        wine_exe = \"TestGame.exe\"";
        let games = parse_config(config).expect("Bad config");
        let game = games.find("test").unwrap();
        match game.env.get("MANGOHUD_CONFIG") {
            Some(s) => assert_eq!(s, "fps_limit=60"),
            None => panic!("No mangohud FPS limit set"),
        }
    }

    #[test]
    fn test_gamescope_height_width_settings() {
        let config = "
        [settings]
        width = 1920
        height = 1080
        use_gamescope = true
        
        [games]
        [games.morrowind]
        name = \"Morrowind\"
        cmd = \"openmw\"
        use_mangohud = true";
        let games = parse_config(config).expect("Bad config");
        if let Some(game) = games.find("morrowind") {
            assert_eq!(
                game.command,
                vec![
                    "gamescope",
                    "-W",
                    "1920",
                    "-H",
                    "1080",
                    "-f",
                    "--force-grab-cursor",
                    "--mangoapp",
                    "--",
                    "openmw"
                ]
            );
        } else {
            panic!("Game not found");
        }
    }

    #[test]
    fn test_gamescope_default_height_and_width_settings() {
        let config = "
        [settings]
        use_gamescope = true
        
        [games]
        [games.morrowind]
        name = \"Morrowind\"
        cmd = \"openmw\"
        use_mangohud = true";
        let games = parse_config(config).expect("Bad config");
        if let Some(game) = games.find("morrowind") {
            assert_eq!(
                game.command,
                vec![
                    "gamescope",
                    "-W",
                    "1280",
                    "-H",
                    "720",
                    "-f",
                    "--force-grab-cursor",
                    "--mangoapp",
                    "--",
                    "openmw"
                ]
            );
        } else {
            panic!("Game not found");
        }
    }

    #[test]
    fn test_gamescope_frame_rate_limit() {
        let config = "
        [settings]
        width = 1920
        height = 1080
        use_gamescope = true
        
        [games]
        [games.test]
        name = \"Test Game\"
        cmd = \"sh start.sh\"
        fps_limit = 60
        use_mangohud = true";
        let games = parse_config(config).expect("Bad config");
        if let Some(game) = games.find("test") {
            assert_eq!(
                game.command,
                vec![
                    "gamescope",
                    "-W",
                    "1920",
                    "-H",
                    "1080",
                    "-f",
                    "--force-grab-cursor",
                    "-r",
                    "60",
                    "--mangoapp",
                    "--",
                    "sh",
                    "start.sh"
                ]
            );
        } else {
            panic!("Game not found");
        }
    }

    #[test]
    fn test_nonexistent_directory_prefix_results_in_error() {
        let config = "
        [games]
        [games.test]
        name = \"Test Game\"
        dir_prefix = \"bad_dir\"
        cmd = \"sh start.sh\"";
        match parse_config(config) {
            Err(ParseError::NoSuchDirectoryPrefix(i, p)) => {
                assert_eq!(i, "test");
                assert_eq!(p, "bad_dir");
            }
            _ => panic!("Parse should fail with nonexistent directory prefix"),
        }
    }
}
