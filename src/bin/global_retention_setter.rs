use aws_sdk_cloudwatchlogs::types::LogGroup;
use lambda_runtime::{Error as LambdaRuntimeError, LambdaEvent};
use log::{debug, error, info, trace};
use serde_json::{json, Value as JsonValue};
use terraform_aws_default_log_retention::global::initialize_metrics;
use terraform_aws_default_log_retention::{
    cloudwatch_logs_traits::{DescribeLogGroups, ListTagsForResource, PutRetentionPolicy, TagResource},
    error::{Error, Severity},
    global::{cloudwatch_logs, initialize_logger, log_group_tags, retention},
    metric_publisher::{self, Metric, MetricName},
};

#[derive(Debug, PartialEq, Eq)]
enum UpdateResult {
    AlreadyHasRetention,
    AlreadyTaggedWithRetention,
    Updated,
}

#[tokio::main]
// Ignore for code coverage
#[cfg(not(tarpaulin_include))]
async fn main() -> Result<(), LambdaRuntimeError> {
    initialize_logger();

    let metrics = initialize_metrics();

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
    debug!("Recevied payload: {}. Context: {:?}", event.payload, event.context);
    let client = cloudwatch_logs().await;
    let result = process_all_log_groups(client).await;

    match result {
        Ok(message) => Ok(message),
        Err(error) => {
            error!("ERROR in Lambda function: {}", error);
            Err(error.into())
        }
    }
}

async fn process_all_log_groups(
    cloudwatch_logs_client: impl DescribeLogGroups + ListTagsForResource + PutRetentionPolicy + TagResource,
) -> Result<JsonValue, Error> {
    let mut errors = vec![];
    let mut total_groups = 0;
    let mut updated = 0;
    let mut already_has_retention = 0;
    let mut already_tagged_with_retention = 0;

    let mut next_token: Option<String> = None;
    loop {
        let result = cloudwatch_logs_client.describe_log_groups(None, next_token.take()).await?;

        for log_group in result.log_groups() {
            total_groups += 1;
            match process_log_group(log_group, &cloudwatch_logs_client).await {
                Ok(result) => match result {
                    UpdateResult::AlreadyHasRetention => already_has_retention += 1,
                    UpdateResult::AlreadyTaggedWithRetention => already_tagged_with_retention += 1,
                    UpdateResult::Updated => updated += 1,
                },
                Err(e) => {
                    error!("Failure updating retention: {}", e);
                    errors.push(e);
                }
            }
        }

        match result.next_token {
            Some(token) => next_token = Some(token),
            None => break,
        }
    }

    let metrics = vec![
        Metric::new(MetricName::Total, total_groups),
        Metric::new(MetricName::Updated, updated),
        Metric::new(MetricName::AlreadyHasRetention, already_has_retention),
        Metric::new(MetricName::AlreadyTaggedWithRetention, already_tagged_with_retention),
        Metric::new(MetricName::Errored, errors.len() as u64),
    ];
    metric_publisher::publish_metrics(metrics);

    match errors.is_empty() {
        true => Ok(
            json!({"message": "Success", "totalGroups": total_groups, "updated": updated, "alreadyHasRetention": already_has_retention, "alreadyTaggedWithRetention": already_tagged_with_retention}),
        ),
        false => {
            error!("Failed to update some log group retentions: {:?}", &errors);
            Err(Error {
                message: format!("Failed to update some log group retentions: {:?}", &errors),
                severity: Severity::Error,
            })
        }
    }
}

async fn process_log_group(
    log_group: &LogGroup,
    client: &(impl PutRetentionPolicy + ListTagsForResource + TagResource),
) -> Result<UpdateResult, LambdaRuntimeError> {
    let log_group_arn = log_group.arn().expect("Log group ARN unexpectedly empty.").replace(":*", ""); // Some ARNs (all ARNs?) have :* on the end, but list-tags-for-resource cannot accept that part
    let log_group_name = log_group.log_group_name().expect("Log group name unexpectedly empty.");
    let log_group_retention = log_group.retention_in_days().unwrap_or(0);

    debug!("Working on {}", log_group_arn);

    if log_group_retention != 0 {
        debug!(
            "Log group {} has retention of {} days already. Not setting.",
            log_group_name, log_group_retention
        );
        return Ok(UpdateResult::AlreadyHasRetention);
    }

    let tags = client.list_tags_for_resource(&log_group_arn).await?;
    if let Some(retention) = tags.tags().and_then(|tags| tags.get("retention")) {
        info!(
            "Not setting retention for {} because tag `retention`=`{}` exists on it.",
            log_group_name, retention
        );
        return Ok(UpdateResult::AlreadyTaggedWithRetention);
    }

    let new_retention = retention();
    client.put_retention_policy(log_group_name, new_retention).await?;
    info!("Set retention of {} days on {}.", new_retention, log_group_name);

    if let Some(tags) = log_group_tags() {
        client.tag_resource(&log_group_arn, tags).await?;
        info!("Tagged {}.", log_group_arn);
    }

    Ok(UpdateResult::Updated)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use mockall::{mock, predicate};

    use async_trait::async_trait;
    use aws_sdk_cloudwatchlogs::{
        operation::{
            describe_log_groups::DescribeLogGroupsOutput, list_tags_for_resource::ListTagsForResourceOutput, put_retention_policy::PutRetentionPolicyOutput,
            tag_resource::TagResourceOutput,
        },
        types::{
            error::{DataAlreadyAcceptedException, InvalidOperationException, ResourceAlreadyExistsException},
            LogGroup,
        },
        Error as CloudWatchLogsError,
    };

    use terraform_aws_default_log_retention::cloudwatch_logs_traits::{PutRetentionPolicy, TagResource};

    #[ctor::ctor]
    fn init() {
        std::env::set_var("log_group_tags", "{}");
    }

    #[tokio::test]
    async fn test_process_all_log_group_success() {
        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();
        mock_cloud_watch_logs_client
            .expect_describe_log_groups()
            .with(predicate::eq(None), predicate::eq(None))
            .returning(|_, _| {
                Ok(DescribeLogGroupsOutput::builder()
                    .log_groups(
                        LogGroup::builder()
                            .log_group_name("MyLogGroupWasCreated")
                            .arn("arn:aws:logs:123:us-west-2:log-group/MyLogGroupWasCreated:*")
                            .retention_in_days(0)
                            .build(),
                    )
                    .log_groups(
                        LogGroup::builder()
                            .log_group_name("AnotherOneWithoutRetention")
                            .arn("arn:aws:logs:123:us-west-2:log-group/AnotherOneWithoutRetention:*")
                            .retention_in_days(0)
                            .build(),
                    )
                    .next_token("NextOnesPlease")
                    .build())
            });
        mock_cloud_watch_logs_client
            .expect_describe_log_groups()
            .with(predicate::eq(None), predicate::eq(Some("NextOnesPlease".to_string())))
            .returning(|_, _| {
                Ok(DescribeLogGroupsOutput::builder()
                    .log_groups(
                        LogGroup::builder()
                            .log_group_name("SecondLogGroupAlreadyHasRetention")
                            .arn("arn:aws:logs:123:us-west-2:log-group/SecondLogGroupAlreadyHasRetention:*")
                            .retention_in_days(90)
                            .build(),
                    )
                    .build())
            });

        mock_cloud_watch_logs_client
            .expect_list_tags_for_resource()
            .returning(|_| Ok(ListTagsForResourceOutput::builder().build()));

        mock_cloud_watch_logs_client
            .expect_put_retention_policy()
            .with(predicate::eq("MyLogGroupWasCreated"), predicate::eq(30))
            .once()
            .returning(|_, _| Ok(PutRetentionPolicyOutput::builder().build()));

        mock_cloud_watch_logs_client
            .expect_put_retention_policy()
            .with(predicate::eq("AnotherOneWithoutRetention"), predicate::eq(30))
            .once()
            .returning(|_, _| Ok(PutRetentionPolicyOutput::builder().build()));

        mock_cloud_watch_logs_client
            .expect_tag_resource()
            .with(
                predicate::eq("arn:aws:logs:123:us-west-2:log-group/MyLogGroupWasCreated"),
                predicate::eq(HashMap::new()),
            )
            .once()
            .returning(|_, _| Ok(TagResourceOutput::builder().build()));

        mock_cloud_watch_logs_client
            .expect_tag_resource()
            .with(
                predicate::eq("arn:aws:logs:123:us-west-2:log-group/AnotherOneWithoutRetention"),
                predicate::eq(HashMap::new()),
            )
            .once()
            .returning(|_, _| Ok(TagResourceOutput::builder().build()));

        let result = process_all_log_groups(mock_cloud_watch_logs_client).await.expect("Should not fail");

        insta::assert_snapshot!(result);
    }

    #[tokio::test]
    async fn test_process_all_log_group_single_already_tagged_with_retention() {
        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();
        mock_cloud_watch_logs_client.expect_describe_log_groups().returning(|_, _| {
            Ok(DescribeLogGroupsOutput::builder()
                .log_groups(
                    LogGroup::builder()
                        .log_group_name("MyLogGroupWasCreated")
                        .arn("arn:aws:logs:123:us-west-2:log-group/NoRetentionAndGetTagsCallFails:*")
                        .retention_in_days(0)
                        .build(),
                )
                .build())
        });

        mock_cloud_watch_logs_client
            .expect_list_tags_for_resource()
            .returning(|_| Ok(ListTagsForResourceOutput::builder().tags("retention", "DoNotTouch").build()));

        let result = process_all_log_groups(mock_cloud_watch_logs_client).await.expect("Should not fail");

        insta::assert_snapshot!(result);
    }

    #[tokio::test]
    async fn test_process_all_log_group_partial_success() {
        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();
        mock_cloud_watch_logs_client
            .expect_describe_log_groups()
            .with(predicate::eq(None), predicate::eq(None))
            .returning(|_, _| {
                Ok(DescribeLogGroupsOutput::builder()
                    .log_groups(
                        LogGroup::builder()
                            .log_group_name("MyLogGroupWasCreated")
                            .arn("arn:aws:logs:123:us-west-2:log-group/MyLogGroupWasCreated:*")
                            .retention_in_days(0)
                            .build(),
                    )
                    .log_groups(
                        LogGroup::builder()
                            .log_group_name("AnotherOneWithoutRetention")
                            .arn("arn:aws:logs:123:us-west-2:log-group/AnotherOneWithoutRetention:*")
                            .retention_in_days(0)
                            .build(),
                    )
                    .log_groups(
                        LogGroup::builder()
                            .log_group_name("NoRetentionAndGetTagsCallFails")
                            .arn("arn:aws:logs:123:us-west-2:log-group/NoRetentionAndGetTagsCallFails:*")
                            .retention_in_days(0)
                            .build(),
                    )
                    .next_token("MoreToCome")
                    .build())
            });
        mock_cloud_watch_logs_client
            .expect_describe_log_groups()
            .with(predicate::eq(None), predicate::eq(Some("MoreToCome".to_string())))
            .returning(|_, _| {
                Ok(DescribeLogGroupsOutput::builder()
                    .log_groups(
                        LogGroup::builder()
                            .log_group_name("SecondLogGroupAlreadyHasRetention")
                            .arn("arn:aws:logs:123:us-west-2:log-group/SecondLogGroupAlreadyHasRetention:*")
                            .retention_in_days(90)
                            .build(),
                    )
                    .build())
            });

        mock_cloud_watch_logs_client
            .expect_list_tags_for_resource()
            .with(predicate::eq("arn:aws:logs:123:us-west-2:log-group/NoRetentionAndGetTagsCallFails"))
            .returning(|_| {
                Err(CloudWatchLogsError::DataAlreadyAcceptedException(
                    DataAlreadyAcceptedException::builder().build(),
                ))
            });

        mock_cloud_watch_logs_client
            .expect_list_tags_for_resource()
            .with(predicate::ne("arn:aws:logs:123:us-west-2:log-group/NoRetentionAndGetTagsCallFails"))
            .returning(|_| Ok(ListTagsForResourceOutput::builder().build()));

        mock_cloud_watch_logs_client
            .expect_put_retention_policy()
            .with(predicate::eq("MyLogGroupWasCreated"), predicate::eq(30))
            .once()
            .returning(|_, _| Ok(PutRetentionPolicyOutput::builder().build()));

        mock_cloud_watch_logs_client
            .expect_put_retention_policy()
            .with(predicate::eq("AnotherOneWithoutRetention"), predicate::eq(30))
            .once()
            .returning(|_, _| Ok(PutRetentionPolicyOutput::builder().build()));

        mock_cloud_watch_logs_client
            .expect_tag_resource()
            .with(
                predicate::eq("arn:aws:logs:123:us-west-2:log-group/MyLogGroupWasCreated"),
                predicate::eq(HashMap::new()),
            )
            .once()
            .returning(|_, _| Ok(TagResourceOutput::builder().build()));

        mock_cloud_watch_logs_client
            .expect_tag_resource()
            .with(
                predicate::eq("arn:aws:logs:123:us-west-2:log-group/AnotherOneWithoutRetention"),
                predicate::eq(HashMap::new()),
            )
            .once()
            .returning(|_, _| Err(CloudWatchLogsError::InvalidOperationException(InvalidOperationException::builder().build())));

        let result = process_all_log_groups(mock_cloud_watch_logs_client).await.expect_err("Should fail");

        insta::assert_snapshot!(result);
    }

    #[tokio::test]
    async fn test_process_log_group_success() {
        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();
        let log_group_arn = "arn:aws:logs:123:us-west-2:log-group/MyLogGroupWasCreated";

        mock_cloud_watch_logs_client
            .expect_list_tags_for_resource()
            .with(predicate::eq(log_group_arn))
            .once()
            .returning(|_| Ok(ListTagsForResourceOutput::builder().build()));

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

        let log_group = LogGroup::builder()
            .log_group_name("MyLogGroupWasCreated")
            .arn("arn:aws:logs:123:us-west-2:log-group/MyLogGroupWasCreated:*")
            .retention_in_days(0)
            .build();

        let result = process_log_group(&log_group, &mock_cloud_watch_logs_client).await.expect("Should not fail");

        assert_eq!(UpdateResult::Updated, result);
    }

    #[tokio::test]
    async fn test_process_log_group_retention_already_set() {
        let mock_cloud_watch_logs_client = MockCloudWatchLogs::new();

        let log_group = LogGroup::builder()
            .log_group_name("MyLogGroupWasCreated")
            .arn("arn:aws:logs:123:us-west-2:log-group/MyLogGroupWasCreated:*")
            .retention_in_days(30)
            .build();

        let result = process_log_group(&log_group, &mock_cloud_watch_logs_client).await.expect("Should not fail");

        assert_eq!(UpdateResult::AlreadyHasRetention, result);
    }

    #[tokio::test]
    async fn test_process_log_group_no_retention_but_tag_present() {
        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();

        mock_cloud_watch_logs_client.expect_list_tags_for_resource().once().returning(|_| {
            Ok(ListTagsForResourceOutput::builder()
                .tags("retention", "I know what I'm doing and I've tagged this group. Leave me alone!")
                .build())
        });

        let log_group = LogGroup::builder()
            .log_group_name("MyLogGroupWasCreated")
            .arn("arn:aws:logs:123:us-west-2:log-group/MyLogGroupWasCreated:*")
            .retention_in_days(0)
            .build();

        let result = process_log_group(&log_group, &mock_cloud_watch_logs_client).await.expect("Should not fail");

        assert_eq!(UpdateResult::AlreadyTaggedWithRetention, result);
    }

    #[tokio::test]
    async fn test_process_log_group_fails() {
        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();
        mock_cloud_watch_logs_client
            .expect_list_tags_for_resource()
            .with(predicate::eq("arn:aws:logs:123:us-west-2:log-group/NoRetentionAndGetTagsCallFails"))
            .once()
            .returning(|_| {
                // This type of error would never happen. Luckily it doesn't matter -- we only care that an error happened.
                Err(CloudWatchLogsError::ResourceAlreadyExistsException(
                    ResourceAlreadyExistsException::builder().build(),
                ))
            });

        let log_group = LogGroup::builder()
            .log_group_name("MyLogGroupWasCreated")
            .arn("arn:aws:logs:123:us-west-2:log-group/NoRetentionAndGetTagsCallFails:*")
            .retention_in_days(0)
            .build();

        let result = process_log_group(&log_group, &mock_cloud_watch_logs_client).await.expect_err("Should fail");

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
                next_token: Option<String>,
            ) -> Result<DescribeLogGroupsOutput, CloudWatchLogsError>;
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
                tags: HashMap<String, String>,
            ) -> Result<TagResourceOutput, CloudWatchLogsError>;
        }

        #[async_trait]
        impl ListTagsForResource for CloudWatchLogs {
            async fn list_tags_for_resource(
                &self,
                resource_arn: &str,
            ) -> Result<ListTagsForResourceOutput, CloudWatchLogsError>;
        }
    }
}
