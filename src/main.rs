use std::collections::HashMap;
use toml::{Table, Value};

fn main() {
    println!("Hello, world!");
}

struct Game {
    id: String,
    name: String,
    command: String,
}

impl Game {
    fn format(&self) -> String {
        format!("{} - {}", self.id, self.name)
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

fn parse_config(config_content: &str) -> Result<Games, ParseError> {
    let mut games = HashMap::new();
    let config = config_content.parse::<Table>().unwrap();
    if let Value::Table(games_config) = &config["games"] {
        for (game_id, value) in games_config.iter() {
            if let Value::Table(game_config) = &value {
                let name = if let Value::String(game_name) = &game_config["name"] {
                    game_name.to_string()
                } else {
                    return Err(ParseError::MissingName(game_id.clone()));
                };
                let command = if let Value::String(cmd) = &game_config["cmd"] {
                    cmd.to_string()
                } else {
                    return Err(ParseError::MissingCommand(game_id.clone()));
                };
                let game = Game {
                    id: game_id.clone(),
                    name,
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
}
