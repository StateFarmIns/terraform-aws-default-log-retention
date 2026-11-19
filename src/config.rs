use std::collections::HashMap;

const DEFAULT_RETENTION_IN_DAYS: i32 = 30;
const DEFAULT_AWS_PARTITION: &str = "aws";
const DEFAULT_METRIC_NAMESPACE: &str = "LogRotation";

/// Runtime configuration, read once from the environment at startup and passed down explicitly.
///
/// Passing this around (instead of reading env vars deep in the call stack) lets tests build a
/// `Config` directly rather than mutating the process environment, which is `unsafe` as of Rust 2024.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub retention_in_days: i32,
    pub log_group_tags: Option<HashMap<String, String>>,
    pub aws_partition: String,
    pub metric_namespace: String,
    pub lambda_function_name: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            retention_in_days: DEFAULT_RETENTION_IN_DAYS,
            log_group_tags: None,
            aws_partition: DEFAULT_AWS_PARTITION.to_string(),
            metric_namespace: DEFAULT_METRIC_NAMESPACE.to_string(),
            lambda_function_name: None,
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Builds a `Config` from any key/value source. `from_env` uses the process environment; tests use a map.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let defaults = Self::default();
        Self {
            retention_in_days: lookup("log_retention_in_days")
                .and_then(|value| value.parse().ok())
                .unwrap_or(defaults.retention_in_days),
            log_group_tags: lookup("log_group_tags").and_then(|value| serde_json::from_str(&value).ok()),
            aws_partition: lookup("aws_partition").unwrap_or(defaults.aws_partition),
            metric_namespace: lookup("metric_namespace").unwrap_or(defaults.metric_namespace),
            lambda_function_name: lookup("AWS_LAMBDA_FUNCTION_NAME"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::Config;

    fn config_from(vars: &[(&str, &str)]) -> Config {
        let vars: HashMap<String, String> = vars.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        Config::from_lookup(|key| vars.get(key).cloned())
    }

    #[test]
    fn test_defaults_when_nothing_set() {
        assert_eq!(config_from(&[]), Config::default());
    }

    #[test]
    fn test_retention() {
        assert_eq!(1, config_from(&[("log_retention_in_days", "1")]).retention_in_days);
    }

    #[test]
    fn test_retention_invalid() {
        assert_eq!(30, config_from(&[("log_retention_in_days", "asdasdasd")]).retention_in_days);
    }

    #[test]
    fn test_retention_not_set() {
        assert_eq!(30, config_from(&[]).retention_in_days);
    }

    #[test]
    fn test_log_group_tags_none() {
        assert_eq!(config_from(&[]).log_group_tags, None);
    }

    #[test]
    fn test_log_group_tags_valid() {
        let config = config_from(&[("log_group_tags", "{\"a\": \"b\", \"c\": \"d\"}")]);

        let mut expected = HashMap::new();
        expected.insert("a".to_string(), "b".to_string());
        expected.insert("c".to_string(), "d".to_string());

        assert_eq!(config.log_group_tags, Some(expected));
    }

    #[test]
    fn test_log_group_tags_invalid_none() {
        assert_eq!(config_from(&[("log_group_tags", "true")]).log_group_tags, None);
    }

    #[test]
    fn test_log_group_tags_empty() {
        assert_eq!(config_from(&[("log_group_tags", "{}")]).log_group_tags, Some(HashMap::new()));
    }

    #[test]
    fn test_aws_partition_and_namespace_and_function_name() {
        let config = config_from(&[
            ("aws_partition", "aws-cn"),
            ("metric_namespace", "Custom"),
            ("AWS_LAMBDA_FUNCTION_NAME", "my-function"),
        ]);

        assert_eq!(config.aws_partition, "aws-cn");
        assert_eq!(config.metric_namespace, "Custom");
        assert_eq!(config.lambda_function_name.as_deref(), Some("my-function"));
    }
}
