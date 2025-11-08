mod game;
use game::{Game, GameError};

use rand::prelude::*;

mod settings;
use settings::Settings;

mod game_builder;
use game_builder::GameBuilder;

mod parse_error;
use parse_error::ParseError;

mod tag;
use tag::TagGroup;

use std::collections::{HashMap, HashSet};
use std::env;
use std::env::{home_dir, var};
use std::fs;
use std::path::PathBuf;
use toml::{Table, Value};

use time::OffsetDateTime;

mod stats;
use stats::GameStats;

const USAGE: &str = "USAGE: game [COMMAND]";
const CONFIG_FILE_NAME: &str = "games.toml";
const DEFAULT_WIDTH: u32 = 1280;
const DEFAULT_HEIGHT: u32 = 720;
const CONFIG_DIR: &str = ".config";
const APP_NAME: &str = "game_rs";
const DATA_DIR: &str = ".local/share/";
const STATS_FILE: &str = "game_stats.tsv";

type CommandHandler = for<'a> fn(games: &'a Games, args: &'a [String]) -> Result<(), GameError<'a>>;

struct GameCommand {
    cmd: &'static str,
    args: Vec<&'static str>,
    exec: CommandHandler,
    desc: &'static str,
}

fn main() {
    // Create the necessary config directory if it doesn't already exist
    match std::fs::create_dir_all(config_dir()) {
        Ok(_) => (),
        Err(e) => {
            println!("Could not create config directory: {}", e);
            std::process::exit(1);
        }
    }

    // Create the necessary datadirectory if it doesn't already exist
    match std::fs::create_dir_all(data_dir()) {
        Ok(_) => (),
        Err(e) => {
            println!("Could not create data directory: {}", e);
            std::process::exit(1);
        }
    }

    let config_contents_result = read_config();
    if config_contents_result.is_err() {
        println!(
            "Error: No {} config file found (expected at $HOME/{}/{}/{})",
            CONFIG_FILE_NAME, CONFIG_DIR, APP_NAME, CONFIG_FILE_NAME
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
                    GameError::NotInstalled => println!("Game is not installed"),
                    GameError::NoEditor => println!("No default editor in $EDITOR"),
                    GameError::CouldNotWriteStats(s) => {
                        println!("Could not write game stats: {}", s)
                    }
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
            ParseError::TomlError(message) => println!("{}", message),
            ParseError::UnrecognizedOption(option) => println!("Unrecognized option: {}", option),
        },
    }
}

fn config_dir() -> PathBuf {
    home_dir().unwrap().join(CONFIG_DIR).join(APP_NAME)
}

fn read_config() -> std::io::Result<String> {
    let config_path = config_dir().join(CONFIG_FILE_NAME);
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
        GameCommand {
            cmd: "play-random",
            args: vec!["TAGS"],
            exec: command_play_random,
            desc: "Play a random game",
        },
        GameCommand {
            cmd: "edit",
            args: Vec::new(),
            exec: command_edit,
            desc: "Edit the config file",
        },
        GameCommand {
            cmd: "stats",
            args: vec!["GAME_ID"],
            exec: command_stats,
            desc: "Show game statistics",
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
    for game in list_games(games, args) {
        println!("{}", game);
    }
    Ok(())
}

fn list_games(games: &Games, args: &[String]) -> Vec<String> {
    let mut game_ids: Vec<&String> = games.games.keys().collect();
    game_ids.sort();

    let tags = &args[0..];

    // List all games having any of the given tags
    game_ids
        .iter()
        .map(|game_id| games.find(game_id).unwrap())
        .filter(|game| game.is_installed())
        .filter(|game| args.is_empty() || game_matches_tags(game, tags))
        .map(|game| game.format())
        .collect()
}

fn game_matches_tags(game: &Game, tag_groups_raw: &[String]) -> bool {
    let tags: Vec<&str> = game.tags.iter().map(|t| t.as_str()).collect();
    tag_groups_raw
        .iter()
        .map(|g| TagGroup::parse(g))
        .any(|tag_group| tag_group.matches(&tags) || tag_group.matches(&[game.id.as_str()]))
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
        Some(game) => play_game(game),
        None => Err(GameError::NoSuchGame(game_id)),
    }
}

fn command_play_random<'a>(games: &'a Games, args: &'a [String]) -> Result<(), GameError<'a>> {
    let game = games.random(args);
    play_game(game)
}

fn play_game<'a>(game: &'a Game) -> Result<(), GameError<'a>> {
    let start_time = OffsetDateTime::now_utc();
    match game.run() {
        Ok(_) => {
            let end_time = OffsetDateTime::now_utc();
            let duration = end_time - start_time;
            let hours = duration.whole_hours();
            let minutes = duration.whole_minutes() - hours * 60;
            let seconds = duration.whole_seconds() - minutes * 60 - hours * 60 * 60;

            let play_time = duration.whole_seconds() as u32;

            println!("Game: {} ({})", game.name, game.id);
            println!(
                "Play Time: {}h{}m{}s ({}sec)",
                hours, minutes, seconds, play_time,
            );

            // Update the stats file
            let mut all_stats: Vec<GameStats> = Vec::new();
            let mut found = false;
            if let Ok(content) = read_stats() {
                for line in content.lines() {
                    if line.is_empty() {
                        continue;
                    }
                    let mut stats = GameStats::from_tsv(line);
                    if stats.id() == game.id {
                        stats.add_time(play_time);
                        stats.update_last_played_time(start_time);
                        found = true;
                    }
                    all_stats.push(stats);
                }
            }

            if !found {
                let stats = GameStats::new(game.id.clone(), play_time, start_time);
                all_stats.push(stats);
            }

            let mut updated_stats = all_stats
                .iter()
                .map(|stats| stats.to_tsv())
                .collect::<Vec<String>>()
                .join("\n");
            updated_stats.push('\n');
            let updated_stats = updated_stats;

            match fs::write(stats_file_path(), updated_stats) {
                Ok(_) => Ok(()),
                Err(e) => Err(GameError::CouldNotWriteStats(e.to_string())),
            }
        }
        Err(e) => Err(e),
    }
}

fn find_game_stats(game: &Game) -> Option<GameStats> {
    if let Ok(content) = read_stats() {
        for line in content.lines() {
            if line.is_empty() {
                continue;
            }
            let stats = GameStats::from_tsv(line);
            if stats.id() == game.id {
                return Some(stats);
            }
        }
        None
    } else {
        None
    }
}

fn read_stats() -> std::io::Result<String> {
    let file_path = stats_file_path();
    fs::read_to_string(&file_path)
}

fn stats_file_path() -> PathBuf {
    data_dir().join(STATS_FILE)
}

fn data_dir() -> PathBuf {
    home_dir().unwrap().join(DATA_DIR).join(APP_NAME)
}

fn command_edit<'a>(_: &'a Games, _: &'a [String]) -> Result<(), GameError<'a>> {
    let config_file_path = config_dir().join(CONFIG_FILE_NAME);
    match var("EDITOR") {
        Ok(editor) => {
            std::process::Command::new(editor)
                .arg(&config_file_path)
                .status()
                .expect("Could nolt edit config file");
            Ok(())
        }
        Err(_) => Err(GameError::NoEditor),
    }
}

fn command_stats<'a>(games: &'a Games, args: &'a [String]) -> Result<(), GameError<'a>> {
    if args.is_empty() {
        return Err(GameError::NoGameId);
    }
    let mut total_seconds = 0;
    let mut count = 0;
    let game_tags = args;
    for game_id in game_tags.iter() {
        match games.find(game_id) {
            Some(game) => match find_game_stats(game) {
                Some(stats) => {
                    count += 1;
                    total_seconds += stats.play_time_seconds();
                    if count > 1 {
                        println!();
                    }
                    println!("{} ({}) Statistics", game.name, game.id);
                    println!("Play Time: {}", stats.format_play_time());
                    println!("Last Played: {}", stats.format_last_played_time());
                }
                None => {
                    if game_tags.len() == 1 {
                        println!("No stats found");
                    }
                }
            },
            None => {
                return Err(GameError::NoSuchGame(game_id));
            }
        }
    }
    if count > 1 {
        let formatted_play_time = stats::format_play_time(total_seconds);
        println!();
        println!("Total Play Time: {}", formatted_play_time);
    }
    Ok(())
}

struct Games {
    games: HashMap<String, Game>,
}

impl Games {
    fn find(&self, id: &str) -> Option<&Game> {
        self.games.get(id)
    }

    fn random(&self, args: &[String]) -> &Game {
        let mut rng = rand::rng();
        let installed_games = self.games.values().filter(|g| g.is_installed());
        let matching_games: Vec<&Game> = if args.is_empty() {
            installed_games.collect()
        } else {
            installed_games
                .filter(|g| game_matches_tags(g, args))
                .collect()
        };
        let games_count = matching_games.len();
        let index = rng.random_range(0..games_count);
        matching_games[index]
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
    let config = match config_content.parse::<Table>() {
        Ok(t) => t,
        Err(e) => return Err(ParseError::TomlError(e.to_string())),
    };

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
                let game = parse_game_config(game_id, game_config, directories, &settings)?;
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

type OptionParser = for<'a, 'b> fn(GameBuilder<'a>, &'b Table) -> GameBuilder<'a>;

fn parse_name<'a>(builder: GameBuilder<'a>, game_config: &Table) -> GameBuilder<'a> {
    if let Value::String(game_name) = &game_config["name"] {
        builder.name(game_name.to_string())
    } else {
        builder
    }
}

fn parse_scummvm_id<'a>(builder: GameBuilder<'a>, game_config: &Table) -> GameBuilder<'a> {
    if let Some(Value::String(scummvm_id)) = game_config.get("scummvm_id") {
        let command = vec!["scummvm".to_string(), scummvm_id.to_string()];
        builder.command(command)
    } else {
        builder
    }
}

fn parse_wine_exe<'a>(builder: GameBuilder<'a>, game_config: &Table) -> GameBuilder<'a> {
    if let Some(Value::String(wine_exe)) = game_config.get("wine_exe") {
        let mut cmd_parts = Vec::new();
        cmd_parts.push("wine".to_string());
        for word in shell_words::split(wine_exe).expect("Failed to parse wine command") {
            cmd_parts.push(word);
        }
        builder.command(cmd_parts)
    } else {
        builder
    }
}

fn parse_dosbox_conf<'a>(builder: GameBuilder<'a>, game_config: &Table) -> GameBuilder<'a> {
    if let Some(Value::String(dosbox_conf_file)) = game_config.get("dosbox_config") {
        let cmd = vec![
            "dosbox".to_string(),
            "-conf".to_string(),
            dosbox_conf_file.to_string(),
        ];
        builder.command(cmd)
    } else {
        builder
    }
}

fn parse_dir_prefix<'a>(builder: GameBuilder<'a>, game_config: &Table) -> GameBuilder<'a> {
    let dir_prefix = game_config.get_str("dir_prefix");
    if !dir_prefix.is_empty() {
        builder.dir_prefix(dir_prefix.to_string())
    } else {
        builder
    }
}

fn parse_cmd<'a>(builder: GameBuilder<'a>, game_config: &Table) -> GameBuilder<'a> {
    if let Some(Value::String(cmd)) = game_config.get("cmd") {
        let command_parts = shell_words::split(cmd).expect("Failed to parse shell command");
        builder.command(command_parts)
    } else {
        builder
    }
}

fn parse_dir<'a>(builder: GameBuilder<'a>, game_config: &Table) -> GameBuilder<'a> {
    if let Some(Value::String(s)) = game_config.get("dir") {
        builder.dir(s.to_string())
    } else {
        builder
    }
}

fn parse_env<'a>(builder: GameBuilder<'a>, game_config: &Table) -> GameBuilder<'a> {
    if let Some(Value::Table(tbl)) = game_config.get("env") {
        let mut environment = HashMap::new();
        for (k, v) in tbl.iter() {
            if let Value::String(s) = v {
                environment.insert(k.clone(), s.as_str().to_string());
            }
        }
        builder.env(environment)
    } else {
        builder
    }
}

fn parse_tags<'a>(builder: GameBuilder<'a>, game_config: &Table) -> GameBuilder<'a> {
    if let Some(Value::Array(tags_array)) = game_config.get("tags") {
        let tags = tags_array
            .iter()
            .filter_map(|x| match x {
                Value::String(tag) => Some(tag.to_string()),
                _ => None,
            })
            .collect();
        builder.tags(tags)
    } else {
        builder
    }
}

fn parse_use_mangohud<'a>(builder: GameBuilder<'a>, game_config: &Table) -> GameBuilder<'a> {
    let use_mangohud = match game_config.get("use_mangohud") {
        Some(Value::Boolean(b)) => *b,
        _ => builder.is_wine(),
    };
    builder.mangohud(use_mangohud)
}

fn parse_fps_limit<'a>(builder: GameBuilder<'a>, game_config: &Table) -> GameBuilder<'a> {
    if let Some(Value::Integer(i)) = game_config.get("fps_limit") {
        builder.fps_limit(*i)
    } else {
        builder
    }
}

fn parse_installed<'a>(builder: GameBuilder<'a>, game_config: &Table) -> GameBuilder<'a> {
    if let Some(Value::Boolean(b)) = game_config.get("installed") {
        if !b { builder.not_installed() } else { builder }
    } else {
        builder
    }
}

fn parse_use_gamescope<'a>(builder: GameBuilder<'a>, game_config: &Table) -> GameBuilder<'a> {
    if let Some(Value::Boolean(b)) = game_config.get("use_gamescope") {
        if *b { builder.use_gamescope() } else { builder }
    } else {
        builder
    }
}

fn parse_use_vk<'a>(builder: GameBuilder<'a>, game_config: &Table) -> GameBuilder<'a> {
    if let Some(Value::Boolean(b)) = game_config.get("use_vk") {
        builder.use_vk(*b)
    } else {
        builder
    }
}

fn parse_game_config(
    game_id: &str,
    game_config: &Table,
    directories: &Table,
    settings: &Settings,
) -> Result<Game, ParseError> {
    let mut option_parsers: HashMap<&str, OptionParser> = HashMap::new();
    option_parsers.insert("cmd", parse_cmd);
    option_parsers.insert("dir", parse_dir);
    option_parsers.insert("dir_prefix", parse_dir_prefix);
    option_parsers.insert("dosbox_config", parse_dosbox_conf);
    option_parsers.insert("env", parse_env);
    option_parsers.insert("fps_limit", parse_fps_limit);
    option_parsers.insert("installed", parse_installed);
    option_parsers.insert("name", parse_name);
    option_parsers.insert("scummvm_id", parse_scummvm_id);
    option_parsers.insert("tags", parse_tags);
    option_parsers.insert("use_gamescope", parse_use_gamescope);
    option_parsers.insert("use_mangohud", parse_use_mangohud);
    option_parsers.insert("use_vk", parse_use_vk);
    option_parsers.insert("wine_exe", parse_wine_exe);
    let option_parsers = option_parsers;

    let mut builder = GameBuilder::new(game_id.to_string(), directories, settings);
    for key in game_config.keys() {
        if !option_parsers.contains_key(key.as_str()) {
            return Err(ParseError::UnrecognizedOption(key.to_string()));
        }
        let parse_option = &option_parsers[key.as_str()];
        builder = parse_option(builder, game_config);
    }

    builder.build()
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
    }

    #[test]
    fn test_wine_game_with_arguments() {
        let config = "
        [games]
        [games.test]
        name = \"Test Game\"
        dir=\"Test Game\"
        use_mangohud = false
        wine_exe = \"'Test Game.exe' -opt1 param1 -opt2\"";
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

    #[test]
    fn test_toml_error() {
        // NOTE: Error is that game ID is duplicated
        let config = "
        [games]
        [games.test]
        name = \"Test Game\"
        dir_prefix = \"bad_dir\"
        cmd = \"sh start.sh\"

        [games.test]
        name = \"Test Game\"
        dir_prefix = \"bad_dir\"
        cmd = \"sh start.sh\"";

        let expected_message = "TOML parse error at line 8, column 16\n  |\n8 |         [games.test]\n  |                ^^^^\nduplicate key\n";
        match parse_config(config) {
            Err(ParseError::TomlError(m)) => assert_eq!(m, expected_message),
            _ => panic!("TOML parse should fail"),
        }
    }

    #[test]
    fn test_dir_from_directories_config() {
        let config = "
        [directories]
        test_game_dir = \"/home/test/test_game\"

        [games]
        
        [games.testgame]
        name = \"Test Game\"
        dir = \"test_game_dir\"
        cmd=\"./test_game\"
        ";

        let games = parse_config(config).expect("Bad config");
        if let Some(game) = games.find("testgame") {
            if let Some(dir) = &game.dir {
                assert_eq!(dir, "/home/test/test_game");
            } else {
                panic!("No directory");
            }
        } else {
            panic!("Game not found");
        }
    }

    #[test]
    fn test_unrecognized_option_produces_error() {
        let config = "
        [games]
        [games.testgame]
        name = \"Test Game\"
        dir = \"test_game_dir\"
        cmd=\"./test_game\"
        use_manohud = true # note the spelling error";
        match parse_config(config) {
            Err(ParseError::UnrecognizedOption(s)) => {
                assert_eq!(s, "use_manohud")
            }
            _ => panic!("This config should produce an error"),
        }
    }

    #[test]
    fn test_do_not_use_vk() {
        let config = "
        [games]
        [games.testgame]
        name = \"Test Game\"
        dir = \"test_game_dir\"
        wine_exe=\"Test.exe\"
        use_vk = false";

        let games = parse_config(config).expect("Bad config");
        if let Some(game) = games.find("testgame") {
            assert_eq!(game.command, vec!["mangohud", "wine", "Test.exe"]);
            match game.env.get("WINEDLLOVERRIDES") {
                Some(s) => assert_eq!(s, "*d3d9,*d3d10,*d3d10_1,*d3d10core,*d3d11,*dxgi=b"),
                None => panic!("No mangohud FPS limit set"),
            }
        } else {
            panic!("Game not found");
        }
    }

    #[test]
    fn test_any_tags_match() {
        let game = Game {
            id: "test_game".to_string(),
            name: "Test Game".to_string(),
            dir: None,
            command: vec!["test_game".to_string()],
            env: HashMap::new(),
            tags: vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()],
            installed: true,
        };
        let tags = ["tag2".to_string(), "tag4".to_string()];
        assert!(game_matches_tags(&game, &tags));
    }

    #[test]
    fn test_all_tags_match() {
        let game = Game {
            id: "test_game".to_string(),
            name: "Test Game".to_string(),
            dir: None,
            command: vec!["test_game".to_string()],
            env: HashMap::new(),
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            installed: true,
        };
        let tags_matching = ["tag1,tag2".to_string()];
        assert!(game_matches_tags(&game, &tags_matching));
        let tags_not_matching = ["tag1,tag3".to_string()];
        assert!(!game_matches_tags(&game, &tags_not_matching));
    }

    #[test]
    fn test_installed_flag_prevents_game_being_played() {
        let config = "
        [games]
        [games.testgame]
        name = \"Test Game\"
        dir = \"test_game_dir\"
        wine_exe=\"Test.exe\"
        installed = false";

        let games = parse_config(config).expect("Bad config");
        if let Some(game) = games.find("testgame") {
            match game.run() {
                Err(GameError::NotInstalled) => (),
                _ => {
                    panic!("Game should not be runnable");
                }
            }
        } else {
            panic!("Game not found");
        }
    }

    #[test]
    fn test_list_does_not_show_games_that_are_not_installed() {
        let config = "
        [games]
        [games.testgame]
        name = \"Test Game\"
        dir = \"test_game_dir\"
        wine_exe=\"Test.exe\"
        installed = false

        [games.testgame2]
        name = \"Test Game 2\"
        dir = \"test_game_dir\"
        wine_exe = \"TestGame2.exe\"";

        let games = parse_config(config).expect("Bad config");
        let game_list = list_games(&games, &[String::new(); 0]);
        assert_eq!(game_list.len(), 1);
        assert_eq!(&game_list[0], "testgame2 - Test Game 2");
    }

    #[test]
    fn test_game_whose_title_matches_the_tag_is_included_in_matches() {
        let game = Game {
            id: "test_game".to_string(),
            name: "Test Game".to_string(),
            dir: None,
            command: vec!["test_game".to_string()],
            env: HashMap::new(),
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            installed: true,
        };
        let tags = vec!["test_game".to_string()];
        assert!(game_matches_tags(&game, &tags));
    }
}
