use std::time::Duration;

use aws_config::{BehaviorVersion, SdkConfig};
use aws_sdk_cloudwatchlogs::Client as CloudWatchLogsClient;
use aws_smithy_types::retry::{RetryConfig, RetryMode};
use cached::proc_macro::cached;
use log::trace;
use metrics_cloudwatch_embedded::Collector;
use tracing::info_span;

use crate::{cloudwatch_logs_traits::CloudWatchLogs, config::Config};

#[cached]
pub async fn cloudwatch_logs() -> CloudWatchLogs {
    let sdk_config = sdk_config().await;
    CloudWatchLogs::new(CloudWatchLogsClient::new(&sdk_config))
}

#[cached]
async fn sdk_config() -> SdkConfig {
    let retry_config = RetryConfig::standard()
        .with_initial_backoff(Duration::from_millis(500))
        .with_max_attempts(10)
        .with_retry_mode(RetryMode::Adaptive);

    aws_config::defaults(BehaviorVersion::v2026_11_17()).retry_config(retry_config).load().await
}

pub fn initialize_logger() {
    trace!("Initializing logger...");
    env_logger::builder().format_timestamp(None).init();
}

pub fn initialize_metrics(config: &Config) -> &'static Collector {
    trace!("Initializing metrics emitter...");

    let lambda_function_name = config
        .lambda_function_name
        .clone()
        .expect("Could not determine Lambda function name. Is this code being run in AWS Lambda?");

    metrics_cloudwatch_embedded::Builder::new()
        .cloudwatch_namespace(config.metric_namespace.clone())
        .with_dimension("function", lambda_function_name)
        .with_lambda_request_id("RequestId")
        .lambda_cold_start_metric("ColdStart")
        .lambda_cold_start_span(info_span!("cold start"))
        .init()
        .expect("Could not instantiate metric emitter.")
}

#[cfg(test)]
mod tests {
    use crate::config::Config;

    use super::{cloudwatch_logs, initialize_logger, initialize_metrics};

    #[test]
    fn test_initialize_metrics() {
        let config = Config {
            lambda_function_name: Some("test-function-name".to_string()),
            ..Config::default()
        };
        initialize_metrics(&config);
    }

    #[test]
    #[should_panic]
    fn test_initialize_metrics_outside_lambda() {
        initialize_metrics(&Config::default());
    }

    #[tokio::test]
    async fn test_cw_logs_client() {
        // Credentials are resolved lazily on first request, so none are needed just to build the client.
        cloudwatch_logs().await;
    }

    #[test]
    fn test_initialize_logger() {
        // Not much to test here......
        initialize_logger();
    }
}
