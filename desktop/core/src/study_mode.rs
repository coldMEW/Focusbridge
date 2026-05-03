use crate::priority::{calculate_score, PriorityConfig, ScoreInput};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudyModeDecision {
    ShowNormally,
    ShowImmediately,
    QueueForBatch,
}

pub struct StudyModeCfg {
    pub enabled: bool,
    pub emergency_override: bool,
}

pub fn decide(
    input: &ScoreInput<'_>,
    priority_cfg: &PriorityConfig,
    study_cfg: &StudyModeCfg,
) -> (StudyModeDecision, i32) {
    let score = calculate_score(input, priority_cfg);
    if !study_cfg.enabled {
        return (StudyModeDecision::ShowNormally, score);
    }
    if score >= 81 {
        return (StudyModeDecision::ShowImmediately, score);
    }
    if score >= 51 && study_cfg.emergency_override {
        return (StudyModeDecision::ShowImmediately, score);
    }
    (StudyModeDecision::QueueForBatch, score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_shows_normally() {
        let cfg = PriorityConfig::default();
        let input = ScoreInput {
            package_name: "x",
            sender: Some("a"),
            message: Some("hi"),
            timestamp_epoch_secs: 0,
            same_sender_msgs_last_5min: 0,
        };
        let (d, _) = decide(
            &input,
            &cfg,
            &StudyModeCfg {
                enabled: false,
                emergency_override: false,
            },
        );
        assert_eq!(d, StudyModeDecision::ShowNormally);
    }

    #[test]
    fn critical_overrides() {
        let cfg = PriorityConfig {
            favorite_contacts: vec!["Mom".into()],
            ..Default::default()
        };
        let input = ScoreInput {
            package_name: "x",
            sender: Some("Mom"),
            message: Some("urgent emergency"),
            timestamp_epoch_secs: 23 * 3600,
            same_sender_msgs_last_5min: 5,
        };
        let (d, _) = decide(
            &input,
            &cfg,
            &StudyModeCfg {
                enabled: true,
                emergency_override: false,
            },
        );
        assert_eq!(d, StudyModeDecision::ShowImmediately);
    }
}
