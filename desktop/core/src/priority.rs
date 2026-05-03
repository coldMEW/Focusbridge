use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PriorityConfig {
    pub favorite_contacts: Vec<String>,
    pub blocked_contacts: Vec<String>,
    pub priority_keywords: Vec<String>,
    pub urgent_keywords: Vec<String>,
    pub priority_apps: Vec<String>,
    pub blocked_apps: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ScoreInput<'a> {
    pub package_name: &'a str,
    pub sender: Option<&'a str>,
    pub message: Option<&'a str>,
    pub timestamp_epoch_secs: i64,
    pub same_sender_msgs_last_5min: u32,
}

pub fn calculate_score(input: &ScoreInput<'_>, cfg: &PriorityConfig) -> i32 {
    let mut score: i32 = 30;
    let msg = input.message.unwrap_or("").to_lowercase();
    let sender = input.sender.unwrap_or("");

    if cfg
        .favorite_contacts
        .iter()
        .any(|c| sender.eq_ignore_ascii_case(c))
    {
        score += 50;
    }
    if cfg
        .blocked_contacts
        .iter()
        .any(|c| sender.eq_ignore_ascii_case(c))
    {
        score -= 10;
    }

    for kw in &cfg.urgent_keywords {
        if msg.contains(&kw.to_lowercase()) {
            score += 30;
        }
    }
    for kw in &cfg.priority_keywords {
        if msg.contains(&kw.to_lowercase()) {
            score += 20;
        }
    }
    if detect_2fa(&msg) {
        score += 100;
    }

    if cfg.priority_apps.iter().any(|p| p == input.package_name) {
        score += 15;
    }
    if cfg.blocked_apps.iter().any(|p| p == input.package_name) {
        score -= 20;
    }

    let hour = hour_of_day(input.timestamp_epoch_secs);
    let is_favorite = cfg
        .favorite_contacts
        .iter()
        .any(|c| sender.eq_ignore_ascii_case(c));
    if is_favorite && (hour >= 22 || hour <= 6) {
        score += 20;
    }

    if input.same_sender_msgs_last_5min >= 3 {
        score += 10;
    }
    if msg.chars().filter(|c| c.is_ascii_digit()).count() >= 10 && msg.contains(char::is_numeric) {
        score += 15;
    }
    if msg.contains("call me") {
        score += 25;
    }

    score.clamp(0, 100)
}

pub fn detect_2fa(lower_msg: &str) -> bool {
    let patterns = [
        ("code", 4..=8),
        ("verify", 4..=8),
        ("verification", 4..=8),
        ("otp", 4..=8),
        ("authentication", 4..=8),
        ("pin", 4..=8),
    ];
    if !lower_msg.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    let run = longest_digit_run(lower_msg);
    for (kw, range) in patterns {
        if lower_msg.contains(kw) && range.contains(&run) {
            return true;
        }
    }
    false
}

fn longest_digit_run(s: &str) -> usize {
    let mut best = 0;
    let mut cur = 0;
    for c in s.chars() {
        if c.is_ascii_digit() {
            cur += 1;
            best = best.max(cur);
        } else {
            cur = 0;
        }
    }
    best
}

fn hour_of_day(ts_secs: i64) -> i64 {
    ((ts_secs.rem_euclid(86400)) / 3600) % 24
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> PriorityConfig {
        PriorityConfig {
            favorite_contacts: vec!["Mom".into()],
            priority_keywords: vec!["urgent".into()],
            urgent_keywords: vec!["emergency".into()],
            priority_apps: vec!["com.whatsapp".into()],
            blocked_apps: vec!["com.instagram.android".into()],
            ..Default::default()
        }
    }

    #[test]
    fn favorite_late_night_critical() {
        let i = ScoreInput {
            package_name: "org.thoughtcrime.securesms",
            sender: Some("Mom"),
            message: Some("urgent emergency"),
            timestamp_epoch_secs: 23 * 3600,
            same_sender_msgs_last_5min: 0,
        };
        assert!(calculate_score(&i, &cfg()) >= 81);
    }

    #[test]
    fn two_fa_reaches_100() {
        let i = ScoreInput {
            package_name: "com.google.authenticator",
            sender: Some("Google"),
            message: Some("Your verification code is 487129"),
            timestamp_epoch_secs: 12 * 3600,
            same_sender_msgs_last_5min: 0,
        };
        assert_eq!(calculate_score(&i, &cfg()), 100);
    }

    #[test]
    fn blocked_app_lowers() {
        let i = ScoreInput {
            package_name: "com.instagram.android",
            sender: Some("stranger"),
            message: Some("liked your photo"),
            timestamp_epoch_secs: 14 * 3600,
            same_sender_msgs_last_5min: 0,
        };
        assert!(calculate_score(&i, &cfg()) < 30);
    }

    #[test]
    fn two_fa_detection() {
        assert!(detect_2fa("your verify code is 123456"));
        assert!(!detect_2fa("hello there"));
        assert!(!detect_2fa("just a 1234567890 phone"));
    }
}
