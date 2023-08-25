use crate::{
    cloudwatch_logs_traits::DescribeLogGroups,
    error::{Error, Severity},
};

pub async fn get_existing_retention(log_group_name: &str, client: &impl DescribeLogGroups) -> Result<i32, Error> {
    let describe_log_groups_response = client.describe_log_groups(Some(log_group_name.to_string()), None).await?;

    let log_group = describe_log_groups_response
        .log_groups()
        .unwrap_or_default()
        .iter()
        .find(|log_group| log_group.log_group_name().unwrap_or_default() == log_group_name);

    match log_group {
        Some(log_group) => Ok(log_group.retention_in_days().unwrap_or(0)),
        None => Err(Error {
            message: format!(
                "Did not find log group named {}. Maybe it was deleted immediately after creation?",
                log_group_name
            ),
            severity: Severity::Warning,
        }),
    }
}

#[cfg(test)]
mod tests {
    use aws_sdk_cloudwatchlogs::{operation::describe_log_groups::DescribeLogGroupsOutput, types::LogGroup, Error as CloudWatchLogsError};
    use mockall::predicate;

    use crate::{cloudwatch_logs_traits::MockDescribeLogGroups, error::Severity};

    use super::get_existing_retention;

    #[tokio::test]
    async fn test_get_existing_retention() {
        let group = "MyLogGroup";
        let retention = 0;

        let mut mock_describe_log_groups = MockDescribeLogGroups::new();
        mock_describe_log_groups
            .expect_describe_log_groups()
            .with(predicate::eq(Some(group.to_string())), predicate::eq(None))
            .returning(move |_, _| mock_describe_log_groups_response(group, retention))
            .once();

        assert_eq!(retention, get_existing_retention(group, &mock_describe_log_groups).await.unwrap());

        let retention = 30;
        mock_describe_log_groups
            .expect_describe_log_groups()
            .with(predicate::eq(Some(group.to_string())), predicate::eq(None))
            .returning(move |_, _| mock_describe_log_groups_response(group, retention))
            .once();

        assert_eq!(retention, get_existing_retention(group, &mock_describe_log_groups).await.unwrap());
    }

    #[tokio::test]
    async fn test_no_log_groups_found_results_in_warning() {
        let group = "MyLogGroup";

        let mut mock_describe_log_groups = MockDescribeLogGroups::new();
        mock_describe_log_groups
            .expect_describe_log_groups()
            .with(predicate::eq(Some(group.to_string())), predicate::eq(None))
            .returning(|_, _| Ok(DescribeLogGroupsOutput::builder().build()))
            .once();

        let err = get_existing_retention(group, &mock_describe_log_groups).await.unwrap_err();

        assert_eq!(Severity::Warning, err.severity);
        assert!(err.message.contains(group));
    }

    #[tokio::test]
    async fn test_wrong_log_groups_found_results_in_warning() {
        let group = "MyLogGroup";

        let mut mock_describe_log_groups = MockDescribeLogGroups::new();
        mock_describe_log_groups
            .expect_describe_log_groups()
            .with(predicate::eq(Some(group.to_string())), predicate::eq(None))
            .returning(|_, _| mock_describe_log_groups_response("SomeRandomOtherLogGroupThatIDidNotAskFor", 0))
            .once();

        let err = get_existing_retention(group, &mock_describe_log_groups).await.unwrap_err();

        assert_eq!(Severity::Warning, err.severity);
        assert!(err.message.contains(group));
    }

    fn mock_describe_log_groups_response(log_group_name: &str, retention: i32) -> Result<DescribeLogGroupsOutput, CloudWatchLogsError> {
        let log_group = LogGroup::builder().log_group_name(log_group_name).retention_in_days(retention).build();
        let response = DescribeLogGroupsOutput::builder().log_groups(log_group).build();
        Ok(response)
    }
}
