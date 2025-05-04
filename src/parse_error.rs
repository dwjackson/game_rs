#[derive(Debug)]
pub enum ParseError {
    MissingName(String),
    MissingCommand(String),
    GameNotTable,
    MissingGameTable,
    NoSuchDirectoryPrefix(String, String),
    TomlError(String),
    UnrecognizedOption(String),
}
