data "aws_lambda_function" "datadog" {
  function_name = "sf-datadog-lambda-forwarder"
}

resource "aws_cloudwatch_log_group" "log_retention_lambda" {
  name              = "/aws/lambda/${local.log_retention_lambda_name}"
  retention_in_days = 30
  tags              = var.tags
}

resource "aws_cloudwatch_log_subscription_filter" "log_retention_lambda_datadog" {
  count           = var.enable_datadog_log_subscription ? 1 : 0
  name            = "default"
  destination_arn = data.aws_lambda_function.datadog.arn
  log_group_name  = aws_cloudwatch_log_group.log_retention_lambda.name
  filter_pattern  = ""
}

resource "aws_cloudwatch_log_group" "global_log_retention_lambda" {
  name              = "/aws/lambda/${local.global_log_retention_lambda_name}"
  retention_in_days = 30
  tags              = var.tags
}

resource "aws_cloudwatch_log_subscription_filter" "global_log_retention_lambda_datadog" {
  count           = var.enable_datadog_log_subscription ? 1 : 0
  name            = "default"
  destination_arn = data.aws_lambda_function.datadog.arn
  log_group_name  = aws_cloudwatch_log_group.global_log_retention_lambda.name
  filter_pattern  = ""
}
