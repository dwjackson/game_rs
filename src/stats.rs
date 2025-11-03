use time::{OffsetDateTime, UtcOffset};

const TIMESTAMP_FORMAT: &str = "[year]-[month]-[day] [hour]:[minute]:[second]";

pub struct GameStats {
    id: String,
    play_time_seconds: u32,
    last_played_time: OffsetDateTime,
}

impl GameStats {
    pub fn new(id: String, play_time_seconds: u32, last_played_time: OffsetDateTime) -> GameStats {
        GameStats {
            id,
            play_time_seconds,
            last_played_time,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn add_time(&mut self, seconds: u32) {
        self.play_time_seconds = self.play_time_seconds.strict_add(seconds);
    }

    pub fn update_last_played_time(&mut self, date_time: OffsetDateTime) {
        self.last_played_time = date_time;
    }

    pub fn to_tsv(&self) -> String {
        let play_time_format =
            time::format_description::parse(TIMESTAMP_FORMAT).expect("Bad format");
        format!(
            "{}\t{}\t{}",
            self.id,
            self.play_time_seconds,
            self.last_played_time
                .format(&play_time_format)
                .expect("Bad format")
        )
    }

    pub fn from_tsv(line: &str) -> GameStats {
        let parts: Vec<&str> = line.split("\t").collect();
        let timestamp_parts: Vec<&str> = parts[2].split(" ").collect();
        let date_str = &timestamp_parts[0];
        let date_parts: Vec<&str> = date_str.split("-").collect();
        let year = date_parts[0].parse::<i32>().expect("Bad year");
        let month: u8 = date_parts[1].parse().expect("Bad month");
        let day = date_parts[2].parse::<u8>().expect("Bad day");
        let time_str = &timestamp_parts[1];
        let time_parts: Vec<&str> = time_str.split(":").collect();
        let hour = time_parts[0].parse::<u8>().expect("Bad hour");
        let minute = time_parts[1].parse::<u8>().expect("Bad minute");
        let second = time_parts[2].parse::<u8>().expect("Bad second");
        let date =
            time::Date::from_calendar_date(year, time::Month::January.nth_next(month - 1), day)
                .expect("Bad date");
        let time = time::Time::from_hms(hour, minute, second).expect("Bad time");
        let offset = UtcOffset::current_local_offset().expect("Bad offset");
        let last_played_time = OffsetDateTime::new_in_offset(date, time, offset);
        GameStats {
            id: parts[0].to_string(),
            play_time_seconds: parts[1].parse::<u32>().expect("Bad play time"),
            last_played_time,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_play_time() {
        let mut stats = GameStats {
            id: "testgame".to_string(),
            play_time_seconds: 90 * 60,
            last_played_time: OffsetDateTime::now_utc(),
        };
        stats.add_time(75 * 60);
        assert_eq!(stats.play_time_seconds, 90 * 60 + 75 * 60);
    }

    #[test]
    fn test_update_last_played_time() {
        let mut stats = GameStats {
            id: "testgame".to_string(),
            play_time_seconds: 90 * 60,
            last_played_time: OffsetDateTime::now_utc(),
        };
        let t = OffsetDateTime::from_unix_timestamp(1762214646).expect("bad timestamp");
        stats.update_last_played_time(t);
        assert_eq!(stats.last_played_time, t);
    }

    #[test]
    fn test_serialize() {
        let date =
            time::Date::from_calendar_date(2025, time::Month::November, 3).expect("Bad date");
        let time = time::Time::from_hms(19, 7, 0).expect("Bad time");
        let offset = time::UtcOffset::UTC;
        let last_played_time = OffsetDateTime::new_in_offset(date, time, offset);
        let stats = GameStats {
            id: "testgame".to_string(),
            play_time_seconds: 90 * 60,
            last_played_time,
        };
        let s = stats.to_tsv();
        assert_eq!("testgame\t5400\t2025-11-03 19:07:00", s);
    }

    #[test]
    fn test_parse() {
        let line = "testgame\t5400\t2025-11-03 19:07:00";
        let stats = GameStats::from_tsv(line);
        assert_eq!(stats.id, "testgame");
        assert_eq!(stats.play_time_seconds, 5400);

        let date =
            time::Date::from_calendar_date(2025, time::Month::November, 3).expect("Bad date");
        let time = time::Time::from_hms(19, 7, 0).expect("Bad time");
        let offset = time::UtcOffset::current_local_offset().expect("No current offset");
        let last_played_time = OffsetDateTime::new_in_offset(date, time, offset);
        assert_eq!(stats.last_played_time, last_played_time);
    }
}
