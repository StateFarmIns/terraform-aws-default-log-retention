use lambda_runtime::{Context, Error as LambdaRuntimeError, LambdaEvent};

use log::{debug, error, info, trace, warn};
use serde_json::{json, Value as JsonValue};
use terraform_aws_default_log_retention::{
    cloudwatch_logs_traits::{DescribeLogGroups, ListTagsForResource, PutRetentionPolicy, TagResource},
    error::{Error, Severity},
    event::CloudTrailEvent,
    global::{aws_partition, cloudwatch_logs, initialize_logger, log_group_tags, metric_namespace, retention},
    metric_publisher::{self, Metric, MetricName},
    retention_setter::get_existing_retention,
};
use tracing::info_span;

// TODO: Main and func are identical for main.rs and global_retention_setter.rs. How to genericize?
#[tokio::main]
// Ignore for code coverage
#[cfg(not(tarpaulin_include))]
async fn main() -> Result<(), LambdaRuntimeError> {
    trace!("Initializing metrics emitter...");
    let lambda_function_name =
        std::env::var("AWS_LAMBDA_FUNCTION_NAME").expect("Could not determine Lambda function name. Is this code being run in AWS Lambda?");
    let metrics = metrics_cloudwatch_embedded::Builder::new()
        .cloudwatch_namespace(metric_namespace())
        .with_dimension("function", lambda_function_name)
        .with_lambda_request_id("RequestId")
        .lambda_cold_start_metric("ColdStart")
        .lambda_cold_start_span(info_span!("cold start").entered())
        .init()
        .expect("Could not instantiate metric emitter.");

    trace!("Initializing logger...");
    initialize_logger();

    trace!("Getting runtime result...");
    let result = metrics_cloudwatch_embedded::lambda::handler::run(metrics, func).await;

    match result {
        Ok(message) => {
            trace!("Received OK message: {:#?}", message);
            Ok(message)
        }
        Err(error) => {
            error!("ERROR in Lambda main: {}", error);
            Err(error)
        }
    }
}

// Ignore for code coverage
#[cfg(not(tarpaulin_include))]
async fn func(event: LambdaEvent<JsonValue>) -> Result<JsonValue, LambdaRuntimeError> {
    debug!("Received payload: {}. Context: {:?}", event.payload, event.context);
    let cloudwatch_logs = cloudwatch_logs().await;
    let cloud_trail_event = parse_event(event.payload, Some(event.context));
    if let Err(error) = cloud_trail_event {
        return process_error(error);
    }
    let cloud_trail_event = cloud_trail_event.expect("Should be Ok() based on above code.");

    let result = process_event(cloud_trail_event, cloudwatch_logs).await;

    match result {
        Ok(message) => Ok(message),
        Err(error) => process_error(error),
    }
}

/// Returns Ok if error is just a warning
fn process_error(error: Error) -> Result<JsonValue, LambdaRuntimeError> {
    match error.severity {
        Severity::Warning => {
            warn!("WARN in Lambda function: {}", error);
            Ok(json!(error))
        }
        Severity::Error => {
            error!("ERROR in Lambda function: {}", error);
            Err(error.into())
        }
    }
}

async fn process_event(
    event: CloudTrailEvent,
    cloudwatch_logs: impl DescribeLogGroups + ListTagsForResource + PutRetentionPolicy + TagResource,
) -> Result<JsonValue, Error> {
    let log_group_name = event.detail.request_parameters.log_group_name;

    let existing_retention = get_existing_retention(&log_group_name, &cloudwatch_logs).await?;

    if existing_retention != 0 {
        info!(
            "Not setting retention for {} because it is set to {} days already.",
            log_group_name, existing_retention
        );
        metric_publisher::publish_metric(Metric::new(MetricName::AlreadyHasRetention, 1));
        return Ok(json!({
            "message":
                format!(
                    "Not setting retention for {} because it is set to {} days already.",
                    log_group_name, existing_retention
                )
        }));
    }

    let log_group_arn = format!(
        "arn:{}:logs:{}:{}:log-group:{}",
        aws_partition(),
        event.detail.aws_region,
        event.detail.user_identity.account_id,
        log_group_name
    );
    let tags = cloudwatch_logs.list_tags_for_resource(&log_group_arn).await?;
    if let Some(retention) = tags.tags().and_then(|tags| tags.get("retention")) {
        info!(
            "Not setting retention for {} because tag `retention`=`{}` exists on it.",
            log_group_name, retention
        );
        metric_publisher::publish_metric(Metric::new(MetricName::AlreadyTaggedWithRetention, 1));
        return Ok(json!({
            "message":
                format!(
                    "Not setting retention for {} because tag `retention`=`{}` exists on it.",
                    log_group_name, retention
                )
        }));
    }

    cloudwatch_logs.put_retention_policy(&log_group_name, retention()).await?;

    metric_publisher::publish_metric(Metric::new(MetricName::Updated, 1));

    if let Some(tags) = log_group_tags() {
        cloudwatch_logs.tag_resource(&log_group_arn, tags).await?;
    }

    info!("Retention set successfully for {}", log_group_name);
    Ok(json!({"message": "Retention set successfully"}))
}

/// Parses a JsonValue into a CloudTrailEvent
/// Normally we could allow our func to parse the event for us, but it doesn't handle errors gracefully or with enough information.
///
/// # Arguments
///    
/// * `payload` the original payload given by Lambda runtime
/// * `context` Optionally, provide the Context object given by the Lambda runtime. It isn't needed for execution; only to enhance the returned error if the payload fails to parse
fn parse_event(payload: JsonValue, context: Option<Context>) -> Result<CloudTrailEvent, Error> {
    // Must clone payload so we can optionally use it in the error message
    let cloud_trail_event = serde_json::from_value(payload.clone());
    if let Err(error) = cloud_trail_event {
        // Known instances are:
        // * When someone tried to make a group but they don't have access
        return Err(Error {
            severity: Severity::Warning,
            message: format!(
                "Error deserializing input payload. Payload: `{}`. Context: `{:?}`. Error: `{}`.",
                payload, context, error
            ),
        });
    }
    let cloud_trail_event = cloud_trail_event.expect("Cannot be Err based on code above");

    Ok(cloud_trail_event)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use async_trait::async_trait;
    use aws_sdk_cloudwatchlogs::{
        operation::{
            describe_log_groups::DescribeLogGroupsOutput, list_tags_for_resource::ListTagsForResourceOutput, put_retention_policy::PutRetentionPolicyOutput,
            tag_resource::TagResourceOutput,
        },
        types::{error::DataAlreadyAcceptedException, LogGroup},
        Error as CloudWatchLogsError,
    };
    use lambda_runtime::{Context, LambdaEvent};
    use mockall::{mock, predicate};
    use serde_json::json;
    use terraform_aws_default_log_retention::event::CloudTrailEvent;
    use terraform_aws_default_log_retention::{
        cloudwatch_logs_traits::{DescribeLogGroups, ListTagsForResource, PutRetentionPolicy, TagResource},
        error::{Error, Severity},
    };

    use crate::{func, parse_event, process_error, process_event};

    #[ctor::ctor]
    fn init() {
        std::env::set_var("log_group_tags", "{}");
    }

    #[tokio::test]
    async fn test_process_event_success_no_tags() {
        let event = CloudTrailEvent::new("123456789", "us-east-1", "MyLogGroupWasCreated");
        let log_group_arn = "arn:aws:logs:us-east-1:123456789:log-group:MyLogGroupWasCreated";

        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();
        mock_cloud_watch_logs_client
            .expect_describe_log_groups()
            .with(predicate::eq(Some("MyLogGroupWasCreated".to_string())), predicate::eq(None))
            .once()
            .returning(|_, _| mock_describe_log_groups_response("MyLogGroupWasCreated", 0));

        mock_cloud_watch_logs_client
            .expect_list_tags_for_resource()
            .with(predicate::eq(log_group_arn))
            .once()
            .returning(|_| mock_list_tags_for_resource_response(None));

        mock_cloud_watch_logs_client
            .expect_put_retention_policy()
            .with(predicate::eq("MyLogGroupWasCreated"), predicate::eq(30))
            .once()
            .returning(|_, _| Ok(PutRetentionPolicyOutput::builder().build()));

        mock_cloud_watch_logs_client
            .expect_tag_resource()
            .with(predicate::eq(log_group_arn), predicate::eq(HashMap::new()))
            .once()
            .returning(|_, _| Ok(TagResourceOutput::builder().build()));

        let result = process_event(event, mock_cloud_watch_logs_client).await.expect("Should not fail");

        insta::assert_debug_snapshot!(result);
    }

    #[tokio::test]
    // Testing for govcloud or China
    async fn test_process_event_success_no_tags_different_aws_partition() {
        std::env::set_var("aws_partition", "aws-cn");
        let event = CloudTrailEvent::new("123456789", "us-east-1", "MyLogGroupWasCreated");
        let log_group_arn = "arn:aws-cn:logs:us-east-1:123456789:log-group:MyLogGroupWasCreated";

        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();
        mock_cloud_watch_logs_client
            .expect_describe_log_groups()
            .with(predicate::eq(Some("MyLogGroupWasCreated".to_string())), predicate::eq(None))
            .once()
            .returning(|_, _| mock_describe_log_groups_response("MyLogGroupWasCreated", 0));

        mock_cloud_watch_logs_client
            .expect_list_tags_for_resource()
            .with(predicate::eq(log_group_arn))
            .once()
            .returning(|_| mock_list_tags_for_resource_response(None));

        mock_cloud_watch_logs_client
            .expect_put_retention_policy()
            .with(predicate::eq("MyLogGroupWasCreated"), predicate::eq(30))
            .once()
            .returning(|_, _| Ok(PutRetentionPolicyOutput::builder().build()));

        mock_cloud_watch_logs_client
            .expect_tag_resource()
            .with(predicate::eq(log_group_arn), predicate::eq(HashMap::new()))
            .once()
            .returning(|_, _| Ok(TagResourceOutput::builder().build()));

        let result = process_event(event, mock_cloud_watch_logs_client).await.expect("Should not fail");

        std::env::remove_var("aws_partition");
        insta::assert_debug_snapshot!(result);
    }

    #[tokio::test]
    async fn test_process_event_fails_when_put_retention_policy_fails() {
        let event = CloudTrailEvent::new("123456789", "us-east-1", "MyLogGroupWasCreated");

        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();
        mock_cloud_watch_logs_client
            .expect_describe_log_groups()
            .with(predicate::eq(Some("MyLogGroupWasCreated".to_string())), predicate::eq(None))
            .once()
            .returning(|_, _| mock_describe_log_groups_response("MyLogGroupWasCreated", 0));

        mock_cloud_watch_logs_client
            .expect_list_tags_for_resource()
            .with(predicate::eq("arn:aws:logs:us-east-1:123456789:log-group:MyLogGroupWasCreated"))
            .once()
            .returning(|_| mock_list_tags_for_resource_response(None));

        mock_cloud_watch_logs_client
            .expect_put_retention_policy()
            .with(predicate::eq("MyLogGroupWasCreated"), predicate::eq(30))
            .once()
            .returning(|_, _| {
                Err(CloudWatchLogsError::DataAlreadyAcceptedException(
                    DataAlreadyAcceptedException::builder().build(),
                ))
            });

        let error = process_event(event, mock_cloud_watch_logs_client).await.expect_err("Should fail");

        insta::assert_debug_snapshot!(error);
    }

    #[tokio::test]
    async fn test_process_event_fails_when_tag_log_group_fails() {
        let event = CloudTrailEvent::new("123456789", "us-east-1", "MyLogGroupWasCreated");

        let log_group_arn = "arn:aws:logs:us-east-1:123456789:log-group:MyLogGroupWasCreated";

        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();
        mock_cloud_watch_logs_client
            .expect_describe_log_groups()
            .with(predicate::eq(Some("MyLogGroupWasCreated".to_string())), predicate::eq(None))
            .once()
            .returning(|_, _| mock_describe_log_groups_response("MyLogGroupWasCreated", 0));

        mock_cloud_watch_logs_client
            .expect_list_tags_for_resource()
            .with(predicate::eq(log_group_arn))
            .once()
            .returning(|_| mock_list_tags_for_resource_response(None));

        mock_cloud_watch_logs_client
            .expect_put_retention_policy()
            .with(predicate::eq("MyLogGroupWasCreated"), predicate::eq(30))
            .once()
            .returning(|_, _| Ok(PutRetentionPolicyOutput::builder().build()));

        mock_cloud_watch_logs_client
            .expect_tag_resource()
            .with(predicate::eq(log_group_arn), predicate::eq(HashMap::new()))
            .once()
            .returning(|_, _| {
                // This type of error would never happen because it is "my" error type rather than an AWS error type. Luckily it doesn't matter -- we only care that an error happened.
                Err(CloudWatchLogsError::DataAlreadyAcceptedException(
                    DataAlreadyAcceptedException::builder().build(),
                ))
            });

        let error = process_event(event, mock_cloud_watch_logs_client).await.expect_err("Should fail");

        insta::assert_debug_snapshot!(error);
    }

    #[tokio::test]
    async fn test_process_event_retention_already_set() {
        let event = CloudTrailEvent::new("123456789", "us-east-1", "MyLogGroupWasCreated");

        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();
        mock_cloud_watch_logs_client
            .expect_describe_log_groups()
            .with(predicate::eq(Some("MyLogGroupWasCreated".to_string())), predicate::eq(None))
            .once()
            .returning(|_, _| mock_describe_log_groups_response("MyLogGroupWasCreated", 30));

        let result = process_event(event, mock_cloud_watch_logs_client).await.expect("Should not fail");

        insta::assert_debug_snapshot!(result);
    }

    #[tokio::test]
    async fn test_process_event_do_not_overwrite_when_retention_tag_set() {
        let event = CloudTrailEvent::new("123456789", "us-east-1", "MyLogGroupWasCreated");

        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();
        mock_cloud_watch_logs_client
            .expect_describe_log_groups()
            .with(predicate::eq(Some("MyLogGroupWasCreated".to_string())), predicate::eq(None))
            .once()
            .returning(|_, _| mock_describe_log_groups_response("MyLogGroupWasCreated", 0));

        mock_cloud_watch_logs_client
            .expect_list_tags_for_resource()
            .with(predicate::eq("arn:aws:logs:us-east-1:123456789:log-group:MyLogGroupWasCreated"))
            .once()
            .returning(|_| mock_list_tags_for_resource_response(Some("Do not override please")));

        let result = process_event(event, mock_cloud_watch_logs_client).await.expect("Should not fail");

        insta::assert_debug_snapshot!(result);
    }

    #[test]
    fn test_parse_event_success() {
        let expected = CloudTrailEvent::new("123", "us-east-77", "SomeLogGroup");
        let input = json!(expected);

        assert_eq!(expected, parse_event(input, None).expect("Should succeed"));
    }

    #[test]
    fn test_parse_event_fail() {
        let input = json!({"invalid": "input"});
        let mut context = Context::default();
        context.request_id = "1231231233123123123".to_string();
        context.invoked_function_arn = "arn:aws:whatever:my-awesome-stuff".to_string();

        let result = parse_event(input, Some(context)).expect_err("Should be an error deserializing the structure");

        insta::assert_debug_snapshot!(result);
    }

    #[test]
    fn test_process_error_severity_error() {
        process_error(Error {
            severity: Severity::Error,
            message: "".to_string(),
        })
        .expect_err("Should be an error");
    }

    #[test]
    fn test_process_error_severity_warning() {
        process_error(Error {
            severity: Severity::Warning,
            message: "".to_string(),
        })
        .expect("Should be successful");
    }

    #[tokio::test]
    async fn test_process_event_bad_input() {
        let input = json!({"invalid": "input"});
        let event = LambdaEvent::new(input, Context::default());
        let result = func(event).await.expect("Should be OK with error message (warning).");

        insta::assert_debug_snapshot!(result);
    }

    // Required to mock multiple traits at a time
    // See https://docs.rs/mockall/latest/mockall/#multiple-and-inherited-traits
    mock! {
        // Creates MockCloudWatchLogs for use in tests
        // Add more trait impls below if needed in tests
        pub CloudWatchLogs {}

        #[async_trait]
        impl DescribeLogGroups for CloudWatchLogs {
            async fn describe_log_groups(
                &self,
                log_group_name_prefix: Option<String>,
                next_token: Option<String>
            ) -> Result<DescribeLogGroupsOutput, CloudWatchLogsError>;
        }

        #[async_trait]
        impl ListTagsForResource for CloudWatchLogs {
            async fn list_tags_for_resource(
                &self,
                resource_arn: &str,
            ) -> Result<ListTagsForResourceOutput, CloudWatchLogsError>;
        }

        #[async_trait]
        impl PutRetentionPolicy for CloudWatchLogs {
            async fn put_retention_policy(
                &self,
                log_group_name: &str,
                retention_in_days: i32,
            ) -> Result<PutRetentionPolicyOutput, CloudWatchLogsError>;
        }

        #[async_trait]
        impl TagResource for CloudWatchLogs {
            async fn tag_resource(
                &self,
                log_group_arn: &str,
                tags: HashMap<String, String>
            ) -> Result<TagResourceOutput, CloudWatchLogsError>;
        }
    }

    #[allow(clippy::result_large_err)] // This is a test, don't care about large err type
    fn mock_describe_log_groups_response(log_group_name: &str, retention: i32) -> Result<DescribeLogGroupsOutput, CloudWatchLogsError> {
        let log_group = LogGroup::builder().log_group_name(log_group_name).retention_in_days(retention).build();
        let response = DescribeLogGroupsOutput::builder().log_groups(log_group).build();
        Ok(response)
    }

    #[allow(clippy::result_large_err)] // This is a test, don't care about large err type
    fn mock_list_tags_for_resource_response(retention_tag_value: Option<&str>) -> Result<ListTagsForResourceOutput, CloudWatchLogsError> {
        if let Some(retention_tag_value) = retention_tag_value {
            let mut tags: HashMap<String, String> = HashMap::new();
            tags.insert("retention".to_string(), retention_tag_value.to_string());
            Ok(ListTagsForResourceOutput::builder().set_tags(Some(tags)).build())
        } else {
            Ok(ListTagsForResourceOutput::builder().build())
        }
    }
}
