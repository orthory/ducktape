use lazy_static::lazy_static;

lazy_static! {
    static ref PATTERN_REGEX: regex::Regex = regex::Regex::new(r"^\/[\w\W]+\{([\w\W]+)}$").unwrap();
}

pub fn parse_variable(input: &str) -> Option<Vec<&str>> {
    match PATTERN_REGEX.captures(input) {
        None => None,
        Some(captures) => Some(captures.get(1).unwrap().as_str().split(";").collect()),
    }
}
