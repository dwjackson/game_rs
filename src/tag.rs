use std::collections::HashSet;

const NOT_PREFIX: &str = "!";

pub struct Tag {
    name: String,
    is_negated: bool,
}

pub struct TagGroup {
    tags: Vec<Tag>,
}

impl TagGroup {
    pub fn parse(s: &str) -> TagGroup {
        let tags = s
            .split(",")
            .map(|tag| {
                let (is_negated, name) = if let Some(stripped_tag) = tag.strip_prefix(NOT_PREFIX) {
                    (true, stripped_tag.to_string())
                } else {
                    (false, tag.to_string())
                };
                Tag { name, is_negated }
            })
            .collect();
        TagGroup { tags }
    }

    pub fn matches(&self, tags: &[&str]) -> bool {
        let mut tag_set: HashSet<&str> = HashSet::new();
        for s in tags.iter() {
            tag_set.insert(s);
        }
        self.tags.iter().all(|tag| {
            tag.is_negated && !tag_set.contains(&tag.name.as_str())
                || !tag.is_negated && tag_set.contains(&tag.name.as_str())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tag_group() {
        let group = TagGroup::parse("tag1,!tag2,tag3");
        assert_eq!(group.tags.len(), 3);
        assert_eq!(group.tags[0].name, "tag1");
        assert!(!group.tags[0].is_negated);
        assert_eq!(group.tags[1].name, "tag2");
        assert!(group.tags[1].is_negated);
    }

    #[test]
    fn test_group_matches() {
        let tags = ["tag1", "tag3"];
        let group = TagGroup::parse("tag1,!tag2,tag3");
        assert!(group.matches(&tags));
    }
}
