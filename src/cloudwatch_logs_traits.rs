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

use async_trait::async_trait;
use aws_sdk_cloudwatchlogs::operation::{
    describe_log_groups::DescribeLogGroupsOutput, list_tags_for_resource::ListTagsForResourceOutput, put_retention_policy::PutRetentionPolicyOutput,
    tag_resource::TagResourceOutput,
};
use aws_sdk_cloudwatchlogs::{Client as CloudWatchLogsClient, Error as CloudWatchLogsError};
use std::collections::HashMap;

#[cfg(test)]
use mockall::automock;

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
    async fn describe_log_groups(
        &self,
        log_group_name_prefix: Option<String>,
        next_token: Option<String>,
    ) -> Result<DescribeLogGroupsOutput, CloudWatchLogsError>;
}

#[async_trait]
pub trait ListTagsForResource {
    async fn list_tags_for_resource(&self, resource_arn: &str) -> Result<ListTagsForResourceOutput, CloudWatchLogsError>;
}

#[async_trait]
pub trait PutRetentionPolicy {
    async fn put_retention_policy(&self, log_group_name: &str, retention_in_days: i32) -> Result<PutRetentionPolicyOutput, CloudWatchLogsError>;
}

#[async_trait]
pub trait TagResource {
    // Add retention tag to a log group
    async fn tag_resource(&self, log_group_arn: &str, tags: HashMap<String, String>) -> Result<TagResourceOutput, CloudWatchLogsError>;
}

/* End Traits */

/* Implementations */

#[async_trait]
impl DescribeLogGroups for CloudWatchLogs {
    async fn describe_log_groups(
        &self,
        log_group_name_prefix: Option<String>,
        next_token: Option<String>,
    ) -> Result<DescribeLogGroupsOutput, CloudWatchLogsError> {
        Ok(self
            .client
            .describe_log_groups()
            .log_group_name_prefix(log_group_name_prefix.unwrap_or_default())
            .next_token(next_token.unwrap_or_default())
            .send()
            .await?)
    }
}

#[async_trait]
impl ListTagsForResource for CloudWatchLogs {
    async fn list_tags_for_resource(&self, resource_arn: &str) -> Result<ListTagsForResourceOutput, CloudWatchLogsError> {
        Ok(self.client.list_tags_for_resource().resource_arn(resource_arn).send().await?)
    }
}

#[async_trait]
impl PutRetentionPolicy for CloudWatchLogs {
    async fn put_retention_policy(&self, log_group_name: &str, retention_in_days: i32) -> Result<PutRetentionPolicyOutput, CloudWatchLogsError> {
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
    async fn tag_resource(&self, log_group_arn: &str, tags: HashMap<String, String>) -> Result<TagResourceOutput, CloudWatchLogsError> {
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
