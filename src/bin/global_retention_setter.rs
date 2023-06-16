use aws_sdk_cloudwatchlogs::model::LogGroup;
use lambda_runtime::{Error as LambdaRuntimeError, LambdaEvent};
use log::{debug, error, info, trace};
use serde_json::{json, Value as JsonValue};
use terraform_aws_default_log_retention::{
    cloudwatch_logs_traits::{DescribeLogGroupsPaginated, ListTagsForResource, PutRetentionPolicy, TagResource},
    cloudwatch_metrics_traits::PutMetricData,
    error::{Error, Severity},
    global::{cloudwatch_logs, cloudwatch_metrics, initialize_logger, log_group_tags, retention},
    metric_publisher::{self, Metric, MetricName},
};
use tokio_stream::StreamExt;

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
    trace!("Initializing logger...");
    initialize_logger();

    trace!("Initializing service function...");
    let func = lambda_runtime::service_fn(func);

    trace!("Getting runtime result...");
    let result = lambda_runtime::run(func).await;

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
    let cloudwatch_metric_client = cloudwatch_metrics().await;
    let result = process_all_log_groups(client, cloudwatch_metric_client).await;

    match result {
        Ok(message) => Ok(message),
        Err(error) => {
            error!("ERROR in Lambda function: {}", error);
            Err(error.into())
        }
    }
}

async fn process_all_log_groups(
    cloudwatch_logs_client: impl DescribeLogGroupsPaginated + ListTagsForResource + PutRetentionPolicy + TagResource,
    cloudwatch_metrics_client: impl PutMetricData,
) -> Result<JsonValue, Error> {
    let mut paginator = cloudwatch_logs_client.describe_log_groups_paginated();

    let mut errors = vec![];
    let mut total_groups = 0;
    let mut updated = 0;
    let mut already_has_retention = 0;
    let mut already_tagged_with_retention = 0;

    while let Some(page) = paginator.next().await {
        let log_groups_page = page.expect("Could not unwrap page; unexpected behavior");
        let log_groups = log_groups_page.log_groups().unwrap_or_default();

        for log_group in log_groups {
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
    }

    let metrics = vec![
        Metric::new(MetricName::Total, total_groups as f64),
        Metric::new(MetricName::Updated, updated as f64),
        Metric::new(MetricName::AlreadyHasRetention, already_has_retention as f64),
        Metric::new(MetricName::AlreadyTaggedWithRetention, already_tagged_with_retention as f64),
        Metric::new(MetricName::Errored, errors.len() as f64),
    ];
    metric_publisher::publish_metrics(cloudwatch_metrics_client, metrics).await;

    match errors.is_empty() {
        true => {
            info!(
                "Success. totalGroups={}, updated={}, alreadyHasRetention={}, alreadyTaggedWithRetention={}",
                total_groups, updated, already_has_retention, already_tagged_with_retention
            );
            Ok(
                json!({"message": "Success", "totalGroups": total_groups, "updated": updated, "alreadyHasRetention": already_has_retention, "alreadyTaggedWithRetention": already_tagged_with_retention}),
            )
        }
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
    use super::*;
    use aws_sdk_cloudwatch::{error::PutMetricDataError, model::MetricDatum, output::PutMetricDataOutput};
    use mockall::{mock, predicate};
    use std::{cell::RefCell, collections::HashMap, pin::Pin};
    use tokio_stream::Stream;

    use async_trait::async_trait;
    use aws_sdk_cloudwatchlogs::{
        error::{DescribeLogGroupsError, ListTagsForResourceError, PutRetentionPolicyError, TagResourceError},
        model::LogGroup,
        output::{DescribeLogGroupsOutput, ListTagsForResourceOutput, PutRetentionPolicyOutput, TagResourceOutput},
        types::SdkError,
    };

    use terraform_aws_default_log_retention::{
        cloudwatch_logs_traits::{PutRetentionPolicy, TagResource},
        cloudwatch_metrics_traits::PutMetricData,
    };

    #[ctor::ctor]
    fn init() {
        std::env::set_var("log_group_tags", "{}");
    }

    #[tokio::test]
    async fn test_process_all_log_group_success() {
        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();
        mock_cloud_watch_logs_client.expect_describe_log_groups_paginated().returning(|| {
            let first_response = DescribeLogGroupsOutput::builder()
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
                .build();
            let second_response = DescribeLogGroupsOutput::builder()
                .log_groups(
                    LogGroup::builder()
                        .log_group_name("SecondLogGroupAlreadyHasRetention")
                        .arn("arn:aws:logs:123:us-west-2:log-group/SecondLogGroupAlreadyHasRetention:*")
                        .retention_in_days(90)
                        .build(),
                )
                .build();
            let events = vec![first_response, second_response];
            let fake_stream = FakeDescribeLogGroupsOutputStream::new(events);

            Box::pin(fake_stream)
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

        let mut mock_cloud_watch_metrics_client = MockCloudWatchMetrics::new();
        mock_cloud_watch_metrics_client
            .expect_put_metric_data()
            .once()
            .withf(|namespace, metrics| {
                assert_eq!("LogRotation", namespace);
                insta::assert_debug_snapshot!("CWMetricCall_process_all_log_group_success", metrics);
                true
            })
            .returning(|_, _| Ok(PutMetricDataOutput::builder().build()));

        let result = process_all_log_groups(mock_cloud_watch_logs_client, mock_cloud_watch_metrics_client)
            .await
            .expect("Should not fail");

        insta::assert_display_snapshot!(result);
    }

    #[tokio::test]
    async fn test_process_all_log_group_single_already_tagged_with_retention() {
        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();
        mock_cloud_watch_logs_client.expect_describe_log_groups_paginated().returning(|| {
            let first_response = DescribeLogGroupsOutput::builder()
                .log_groups(
                    LogGroup::builder()
                        .log_group_name("MyLogGroupWasCreated")
                        .arn("arn:aws:logs:123:us-west-2:log-group/NoRetentionAndGetTagsCallFails:*")
                        .retention_in_days(0)
                        .build(),
                )
                .build();
            let events = vec![first_response];
            let fake_stream = FakeDescribeLogGroupsOutputStream::new(events);

            Box::pin(fake_stream)
        });

        mock_cloud_watch_logs_client
            .expect_list_tags_for_resource()
            .returning(|_| Ok(ListTagsForResourceOutput::builder().tags("retention", "DoNotTouch").build()));

        let mut mock_cloud_watch_metrics_client = MockCloudWatchMetrics::new();
        mock_cloud_watch_metrics_client
            .expect_put_metric_data()
            .once()
            .withf(|namespace, metrics| {
                assert_eq!("LogRotation", namespace);
                insta::assert_debug_snapshot!("CWMetricCall_process_all_log_group_single_already_tagged_with_retention", metrics);
                true
            })
            .returning(|_, _| Ok(PutMetricDataOutput::builder().build()));

        let result = process_all_log_groups(mock_cloud_watch_logs_client, mock_cloud_watch_metrics_client)
            .await
            .expect("Should not fail");

        insta::assert_display_snapshot!(result);
    }

    #[tokio::test]
    async fn test_process_all_log_group_partial_success() {
        let mut mock_cloud_watch_logs_client = MockCloudWatchLogs::new();
        mock_cloud_watch_logs_client.expect_describe_log_groups_paginated().returning(|| {
            let first_response = DescribeLogGroupsOutput::builder()
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
                .build();
            let second_response = DescribeLogGroupsOutput::builder()
                .log_groups(
                    LogGroup::builder()
                        .log_group_name("SecondLogGroupAlreadyHasRetention")
                        .arn("arn:aws:logs:123:us-west-2:log-group/SecondLogGroupAlreadyHasRetention:*")
                        .retention_in_days(90)
                        .build(),
                )
                .build();
            let events = vec![first_response, second_response];
            let fake_stream = FakeDescribeLogGroupsOutputStream::new(events);

            Box::pin(fake_stream)
        });

        mock_cloud_watch_logs_client
            .expect_list_tags_for_resource()
            .with(predicate::eq("arn:aws:logs:123:us-west-2:log-group/NoRetentionAndGetTagsCallFails"))
            .returning(|_| {
                // This type of error would never happen because it is "my" error type rather than an AWS error type. Luckily it doesn't matter -- we only care that an error happened.
                Err(SdkError::timeout_error(Box::new(Error {
                    message: "Some error happened when getting tags".to_string(),
                    severity: Severity::Error,
                })))
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
            .returning(|_,_|
            // This type of error would never happen because it is "my" error type rather than an AWS error type. Luckily it doesn't matter -- we only care that an error happened.
            Err(SdkError::timeout_error(Box::new(Error {
                message: "Some error happened".to_string(),
                severity: Severity::Error,
                            }))));

        let mut mock_cloud_watch_metrics_client = MockCloudWatchMetrics::new();
        mock_cloud_watch_metrics_client
            .expect_put_metric_data()
            .once()
            .withf(|namespace, metrics| {
                assert_eq!("LogRotation", namespace);
                insta::assert_debug_snapshot!("CWMetricCall_process_all_log_group_partial_success", metrics);
                true
            })
            .returning(|_, _| Ok(PutMetricDataOutput::builder().build()));

        let result = process_all_log_groups(mock_cloud_watch_logs_client, mock_cloud_watch_metrics_client)
            .await
            .expect_err("Should fail");

        insta::assert_display_snapshot!(result);
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
                // This type of error would never happen because it is "my" error type rather than an AWS error type. Luckily it doesn't matter -- we only care that an error happened.
                Err(SdkError::timeout_error(Box::new(Error {
                    message: "Some error happened".to_string(),
                    severity: Severity::Error,
                })))
            });

        let log_group = LogGroup::builder()
            .log_group_name("MyLogGroupWasCreated")
            .arn("arn:aws:logs:123:us-west-2:log-group/NoRetentionAndGetTagsCallFails:*")
            .retention_in_days(0)
            .build();

        let result = process_log_group(&log_group, &mock_cloud_watch_logs_client).await.expect_err("Should fail");

        insta::assert_debug_snapshot!(result);
    }

    #[derive(Clone)]
    struct FakeDescribeLogGroupsOutputStream {
        results: RefCell<Vec<DescribeLogGroupsOutput>>,
    }

    impl FakeDescribeLogGroupsOutputStream {
        fn new(results: Vec<DescribeLogGroupsOutput>) -> Self {
            Self {
                results: RefCell::new(results),
            }
        }
    }

    impl Stream for FakeDescribeLogGroupsOutputStream {
        type Item = Result<DescribeLogGroupsOutput, SdkError<DescribeLogGroupsError>>;

        fn poll_next(self: Pin<&mut Self>, _: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
            let mut results = self.results.borrow_mut();
            match results.is_empty() {
                true => std::task::Poll::Ready(None),
                false => std::task::Poll::Ready(Some(Ok(results.pop().unwrap()))),
            }
        }
    }

    // Required to mock multiple traits at a time
    // See https://docs.rs/mockall/latest/mockall/#multiple-and-inherited-traits
    mock! {
        // Creates MockCloudWatchLogs for use in tests
        // Add more trait impls below if needed in tests
        pub CloudWatchLogs {}

        #[async_trait]
        impl DescribeLogGroupsPaginated for CloudWatchLogs {
            fn describe_log_groups_paginated(
                &self,
            ) -> Pin<
                Box<dyn Stream<Item = Result<DescribeLogGroupsOutput, SdkError<DescribeLogGroupsError>>>>>;
        }

        #[async_trait]
        impl PutRetentionPolicy for CloudWatchLogs {
            async fn put_retention_policy(
                &self,
                log_group_name: &str,
                retention_in_days: i32,
            ) -> Result<PutRetentionPolicyOutput, SdkError<PutRetentionPolicyError>>;
        }

        #[async_trait]
        impl TagResource for CloudWatchLogs {
            async fn tag_resource(
                &self,
                log_group_arn: &str,
                tags: HashMap<String, String>,
            ) -> Result<TagResourceOutput, SdkError<TagResourceError>>;
        }

        #[async_trait]
        impl ListTagsForResource for CloudWatchLogs {
            async fn list_tags_for_resource(
                &self,
                resource_arn: &str,
            ) -> Result<ListTagsForResourceOutput, SdkError<ListTagsForResourceError>>;
        }
    }

    mock! {
        pub CloudWatchMetrics {}

        #[async_trait]
        impl PutMetricData for CloudWatchMetrics {
            async fn put_metric_data(
                &self,
                namespace: String,
                metric_data: Vec<MetricDatum>,
            ) -> Result<PutMetricDataOutput, SdkError<PutMetricDataError>>;
        }
    }
}
