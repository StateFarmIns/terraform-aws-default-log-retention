locals {
  alarms = {
    retention-errors = {
      description         = "Errors occurred invoking the ${aws_lambda_function.log_retention.function_name} function! Log groups are not getting default retention.",
      comparison_operator = "GreaterThanThreshold"
      threshold           = aws_lambda_function_event_invoke_config.log_retention.maximum_retry_attempts # Set to value of max retry attempts (2) because if all attempts fail we will see `retry attempts + 1 primary attempt` failures (3). Alarm only when all fail.
      period              = (aws_lambda_function.log_retention.timeout * (aws_lambda_function_event_invoke_config.log_retention.maximum_retry_attempts + 1)) + 60
      metric_name         = "Errors"
      function_name       = aws_lambda_function.log_retention.function_name
    }
    retention-throttles = {
      description         = "The ${aws_lambda_function.log_retention.function_name} function was throttled! Log groups are not getting default retention.",
      comparison_operator = "GreaterThanThreshold"
      threshold           = 0
      period              = 300
      metric_name         = "Throttles"
      function_name       = aws_lambda_function.log_retention.function_name
    }
    global-retention-errors = {
      description         = "Errors occurred invoking the ${aws_lambda_function.global_log_retention.function_name} function! Log groups are not getting default retention.",
      comparison_operator = "GreaterThanThreshold"
      threshold           = aws_lambda_function_event_invoke_config.global_log_retention.maximum_retry_attempts # Set to value of max retry attempts (2) because if all attempts fail we will see `retry attempts + 1 primary attempt` failures (3). Alarm only when all fail.
      period              = (aws_lambda_function.global_log_retention.timeout * (aws_lambda_function_event_invoke_config.global_log_retention.maximum_retry_attempts + 1)) + 60
      metric_name         = "Errors"
      function_name       = aws_lambda_function.global_log_retention.function_name
    }
    global-retention-throttles = {
      description         = "The ${aws_lambda_function.global_log_retention.function_name} function was throttled! Log groups are not getting default retention.",
      comparison_operator = "GreaterThanThreshold"
      threshold           = 0
      period              = 300
      metric_name         = "Throttles"
      function_name       = aws_lambda_function.global_log_retention.function_name
    }
  }
}

resource "aws_cloudwatch_metric_alarm" "alarm" {
  for_each            = local.enable_alarms ? local.alarms : []
  alarm_name          = "${var.name}-${each.key}"
  alarm_description   = each.value.description
  comparison_operator = each.value.comparison_operator
  threshold           = each.value.threshold
  evaluation_periods  = 1
  metric_name         = each.value.metric_name
  namespace           = "AWS/Lambda"
  period              = each.value.period
  statistic           = "Sum"
  actions_enabled     = true

  alarm_actions             = [local.sns_topic_arn]
  ok_actions                = []
  insufficient_data_actions = []

  dimensions = { FunctionName = each.value.function_name }

  tags = var.tags
}
