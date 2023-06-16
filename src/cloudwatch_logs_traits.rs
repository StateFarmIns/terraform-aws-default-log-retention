// Traits defined for testing purposes -- see https://docs.aws.amazon.com/sdk-for-rust/latest/dg/testing.html

/*

This file contains a wrapper struct `CloudWatchLogs` which wraps the default CloudWatch Logs Client.
Traits are defined in the middle of the file to provide generic interfaces to each CW Logs operation.
At the bottom, default implementations are provided to invoke the main client.

For testing, you can automock each of these traits; much easier than creating a fake AWS API HTTP server!

Adding a new operation:
1. First add a new trait. Add the `#[cfg_attr(test, automock)]` annotation before it, which MUST appear before the `#[async_trait]` annotation (per automock documentation)
2. Add a default implementation in the bottom section that invokes the AWS CW Logs client.
3. Test examples are in `retention_setter.rs`

*/

use std::{collections::HashMap, pin::Pin};

use async_trait::async_trait;
use aws_sdk_cloudwatchlogs::{
    error::{DescribeLogGroupsError, ListTagsForResourceError, PutRetentionPolicyError, TagResourceError},
    output::{DescribeLogGroupsOutput, ListTagsForResourceOutput, PutRetentionPolicyOutput, TagResourceOutput},
    types::SdkError,
    Client as CloudWatchLogsClient,
};
#[cfg(test)]
use mockall::automock;
use tokio_stream::Stream;

/* Base Struct */

#[derive(Clone, Debug)]
pub struct CloudWatchLogs {
    client: CloudWatchLogsClient,
}

impl CloudWatchLogs {
    pub fn new(client: CloudWatchLogsClient) -> Self {
        Self { client }
    }
}

/* End Base Struct */

/* Traits */

#[cfg_attr(test, automock)]
#[async_trait]
pub trait DescribeLogGroups {
    async fn describe_log_groups(&self, log_group_name_prefix: &str) -> Result<DescribeLogGroupsOutput, SdkError<DescribeLogGroupsError>>;
}

#[async_trait]
pub trait DescribeLogGroupsPaginated {
    fn describe_log_groups_paginated(&self) -> Pin<Box<dyn Stream<Item = Result<DescribeLogGroupsOutput, SdkError<DescribeLogGroupsError>>>>>;
}

#[async_trait]
pub trait ListTagsForResource {
    async fn list_tags_for_resource(&self, resource_arn: &str) -> Result<ListTagsForResourceOutput, SdkError<ListTagsForResourceError>>;
}

#[async_trait]
pub trait PutRetentionPolicy {
    async fn put_retention_policy(&self, log_group_name: &str, retention_in_days: i32) -> Result<PutRetentionPolicyOutput, SdkError<PutRetentionPolicyError>>;
}

#[async_trait]
pub trait TagResource {
    // Add retention tag to a log group
    async fn tag_resource(&self, log_group_arn: &str, tags: HashMap<String, String>) -> Result<TagResourceOutput, SdkError<TagResourceError>>;
}

/* End Traits */

/* Implementations */

#[async_trait]
impl DescribeLogGroups for CloudWatchLogs {
    async fn describe_log_groups(&self, log_group_name_prefix: &str) -> Result<DescribeLogGroupsOutput, SdkError<DescribeLogGroupsError>> {
        Ok(self.client.describe_log_groups().log_group_name_prefix(log_group_name_prefix).send().await?)
    }
}

#[async_trait]
impl DescribeLogGroupsPaginated for CloudWatchLogs {
    fn describe_log_groups_paginated(&self) -> Pin<Box<dyn Stream<Item = Result<DescribeLogGroupsOutput, SdkError<DescribeLogGroupsError>>>>> {
        Box::pin(self.client.describe_log_groups().into_paginator().send())
    }
}

#[async_trait]
impl ListTagsForResource for CloudWatchLogs {
    async fn list_tags_for_resource(&self, resource_arn: &str) -> Result<ListTagsForResourceOutput, SdkError<ListTagsForResourceError>> {
        Ok(self.client.list_tags_for_resource().resource_arn(resource_arn).send().await?)
    }
}

#[async_trait]
impl PutRetentionPolicy for CloudWatchLogs {
    async fn put_retention_policy(&self, log_group_name: &str, retention_in_days: i32) -> Result<PutRetentionPolicyOutput, SdkError<PutRetentionPolicyError>> {
        Ok(self
            .client
            .put_retention_policy()
            .log_group_name(log_group_name)
            .retention_in_days(retention_in_days)
            .send()
            .await?)
    }
}

#[async_trait]
impl TagResource for CloudWatchLogs {
    async fn tag_resource(&self, log_group_arn: &str, tags: HashMap<String, String>) -> Result<TagResourceOutput, SdkError<TagResourceError>> {
        Ok(self
            .client
            .tag_resource()
            .resource_arn(log_group_arn)
            .set_tags(Some(tags))
            .tags("retention", "Set by AWS Default Log Retention project.")
            .send()
            .await?)
    }
}

/* End Implementations */
