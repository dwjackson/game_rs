use std::collections::HashMap;
use toml::{Table, Value};

fn main() {
    println!("Hello, world!");
}

struct Game {
    id: String,
}

impl Game {
    fn format(&self) -> String {
        self.id.to_string()
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

fn parse_config(config_content: &str) -> Games {
    let mut games = HashMap::new();
    let config = config_content.parse::<Table>().unwrap();
    if let Value::Table(games_config) = &config["games"] {
        for (game_id, value) in games_config.iter() {
            if let Value::Table(game_config) = &value {
                let game = Game {
                    id: game_id.clone(),
                };
                games.insert(game_id.clone(), game);
            } else {
                panic!("Game was not a table: {}", game_id);
            }
        }
    } else {
        panic!("No 'games' table found");
    }
    Games {
        games,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_exists() {
        let config = "[games]\n[games.morrowind]\ncmd = \"openmw\"";
        let games = parse_config(config);
        assert!(games.exists("morrowind"));
    }

    #[test]
    fn test_format_game_with_id_but_without_name() {
        let config = "[games]\n[games.morrowind]\ncmd = \"openmw\"";
        let games = parse_config(config);
        if let Some(game) = games.find("morrowind") {
            let s = game.format();
            assert_eq!(s, "morrowind");
        } else {
            panic!("Game not found");
        }
    }
}
