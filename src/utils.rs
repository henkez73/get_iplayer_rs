use regex::Regex;
use std::collections::HashMap;

/// Converts English number words (zero..ninety-nine) or numeric strings to integer.
pub fn words_to_number(text: &str) -> Option<u32> {
    let text = text.trim().to_lowercase();
    if let Ok(num) = text.parse::<u32>() {
        return Some(num);
    }

    let units: HashMap<&str, u32> = [
        ("zero", 0), ("one", 1), ("two", 2), ("three", 3), ("four", 4),
        ("five", 5), ("six", 6), ("seven", 7), ("eight", 8), ("nine", 9),
        ("ten", 10), ("eleven", 11), ("twelve", 12), ("thirteen", 13), ("fourteen", 14),
        ("fifteen", 15), ("sixteen", 16), ("seventeen", 17), ("eighteen", 18), ("nineteen", 19),
    ].into_iter().collect();

    let tens: HashMap<&str, u32> = [
        ("twenty", 20), ("thirty", 30), ("forty", 40), ("fifty", 50),
        ("sixty", 60), ("seventy", 70), ("eighty", 80), ("ninety", 90),
    ].into_iter().collect();

    if let Some(&val) = units.get(text.as_str()) {
        return Some(val);
    }
    if let Some(&val) = tens.get(text.as_str()) {
        return Some(val);
    }

    // Try compound like "twenty-one" or "twenty one"
    let parts: Vec<&str> = text.split(|c: char| c == '-' || c == ' ' || c == '_').filter(|s| !s.is_empty()).collect();
    if parts.len() == 2 {
        if let (Some(&t), Some(&u)) = (tens.get(parts[0]), units.get(parts[1])) {
            return Some(t + u);
        }
    }

    None
}

/// Regex pattern matching numbers or number words
pub const REGEX_NUMBERS_PATTERN: &str = r"(?i)(\d+|zero|one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|(twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety)([\s\-]?(zero|one|two|three|four|five|six|seven|eight|nine))?)";

/// Extracts episode number from episode or synopsis string
pub fn extract_episode_num(text: &str) -> Option<u32> {
    let re1 = Regex::new(&format!(r"(?i)(?:Episode|Pennod)\s+{}", REGEX_NUMBERS_PATTERN)).ok()?;
    if let Some(caps) = re1.captures(text) {
        if let Some(m) = caps.get(1) {
            if let Some(n) = words_to_number(m.as_str()) {
                return Some(n);
            }
        }
    }
    let re2 = Regex::new(&format!(r"(?i)^\s*{}\.\s+", REGEX_NUMBERS_PATTERN)).ok()?;
    if let Some(caps) = re2.captures(text) {
        if let Some(m) = caps.get(1) {
            if let Some(n) = words_to_number(m.as_str()) {
                return Some(n);
            }
        }
    }
    None
}

/// Extracts series number from series name or title string
pub fn extract_series_num(text: &str) -> Option<u32> {
    let re = Regex::new(&format!(r"(?i)(?:Series|Cyfres|Season)\s+{}", REGEX_NUMBERS_PATTERN)).ok()?;
    if let Some(caps) = re.captures(text) {
        if let Some(m) = caps.get(1) {
            if let Some(n) = words_to_number(m.as_str()) {
                return Some(n);
            }
        }
    }
    None
}

/// Sanitizes filename: removes/replaces invalid filesystem characters and optional whitespace
pub fn sanitize_filename(name: &str, keep_whitespace: bool) -> String {
    let mut clean = String::new();
    for c in name.chars() {
        match c {
            '/' | '\\' | '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>' => {
                clean.push('-');
            }
            ' ' | '\t' | '\r' | '\n' => {
                if keep_whitespace {
                    clean.push(c);
                } else {
                    clean.push('_');
                }
            }
            _ => {
                clean.push(c);
            }
        }
    }
    // Collapse consecutive hyphens / underscores / spaces around hyphens
    let re_mix = Regex::new(r"[-_\s]*-[-_\s]*").unwrap();
    let s = re_mix.replace_all(&clean, "-");
    let re_underscores = Regex::new(r"_+").unwrap();
    let s = re_underscores.replace_all(&s, "_");
    let re_spaces = Regex::new(r"[ \t]+").unwrap();
    let s = re_spaces.replace_all(&s, " ");
    s.trim_matches(|c: char| c == ' ' || c == '-' || c == '_').to_string()
}

/// Check if an executable exists in the system PATH
pub fn exists_in_path(cmd: &str) -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        for p in std::env::split_paths(&path_var) {
            let full = p.join(cmd);
            if full.is_file() {
                return true;
            }
            #[cfg(target_os = "windows")]
            {
                let full_exe = p.join(format!("{}.exe", cmd));
                if full_exe.is_file() {
                    return true;
                }
            }
        }
    }
    false
}

/// Formats seconds as HH:MM:SS
pub fn format_duration(seconds: u32) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

/// Parse time string like "01:23:45" or "120" to seconds
pub fn parse_time_offset(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Ok(sec) = s.parse::<u32>() {
        return Some(sec);
    }
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 2 {
        let m: u32 = parts[0].parse().ok()?;
        let s: u32 = parts[1].parse().ok()?;
        return Some(m * 60 + s);
    } else if parts.len() == 3 {
        let h: u32 = parts[0].parse().ok()?;
        let m: u32 = parts[1].parse().ok()?;
        let s: u32 = parts[2].parse().ok()?;
        return Some(h * 3600 + m * 60 + s);
    }
    None
}

/// Expands substitution parameters in templates (like `<name>`, `<episode>`, `<pid>`)
pub fn substitute_template(template: &str, vars: &HashMap<&str, String>) -> String {
    let mut result = template.to_string();
    for (key, val) in vars {
        let placeholder = format!("<{}>", key);
        result = result.replace(&placeholder, val);
    }
    // Remove any leftover unmatched tags <foo>
    let re = Regex::new(r"<[a-zA-Z0-9_\-]+>").unwrap();
    result = re.replace_all(&result, "").to_string();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_words_to_number() {
        assert_eq!(words_to_number("1"), Some(1));
        assert_eq!(words_to_number("one"), Some(1));
        assert_eq!(words_to_number("twenty-one"), Some(21));
        assert_eq!(words_to_number("ninety nine"), Some(99));
        assert_eq!(words_to_number("twelve"), Some(12));
    }

    #[test]
    fn test_extract_episode() {
        assert_eq!(extract_episode_num("Episode 5"), Some(5));
        assert_eq!(extract_episode_num("Episode five"), Some(5));
        assert_eq!(extract_episode_num("3. The Rings of Akhaten"), Some(3));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Doctor Who: Series 7 / Part 2", false), "Doctor_Who-Series_7-Part_2");
        assert_eq!(sanitize_filename("Doctor Who: Series 7 / Part 2", true), "Doctor Who-Series 7-Part 2");
    }
}
