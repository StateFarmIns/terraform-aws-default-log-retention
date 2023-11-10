data "archive_file" "log_retention" {
  type        = "zip"
  source_file = "${path.module}/dist/log_retention_setter/bootstrap"
  output_path = "${path.module}/dist/log_retention_setter/bootstrap.zip"
}

resource "aws_lambda_function" "log_retention" {
  depends_on       = [aws_cloudwatch_log_group.log_retention_lambda]
  function_name    = local.log_retention_lambda_name
  filename         = data.archive_file.log_retention.output_path
  source_code_hash = data.archive_file.log_retention.output_base64sha256
  runtime          = "provided.al2"
  architectures    = ["arm64"]
  handler          = "bootstrap"
  role             = aws_iam_role.log_retention.arn
  timeout          = 60
  kms_key_arn      = var.kms_key_arn
  memory_size      = 128
  description      = "Sets default CloudWatch Logs retention settings for new log groups."

  environment {
    variables = {
      log_retention_in_days = var.log_retention_in_days
      log_group_tags        = local.log_group_tags_json
      metric_namespace      = var.metric_namespace
      aws_partition         = data.aws_partition.current.partition
      RUST_BACKTRACE        = 1
      RUST_LOG              = "warn,terraform_aws_default_log_retention=${var.log_level}" # https://docs.rs/env_logger/latest/env_logger/
    }
  }

  dynamic "vpc_config" {
    for_each = var.subnet_ids == null ? [] : ["make this block once"]
    content {
      subnet_ids         = var.subnet_ids
      security_group_ids = [var.https_egress_security_group_id]
    }
  }

  tags = var.tags
}

resource "aws_cloudwatch_event_rule" "log_group_creation" {
  name = "${var.name}-log-group-creation"

  event_pattern = <<PATTERN
{
  "source": [
    "aws.logs"
  ],
  "detail-type": [
    "AWS API Call via CloudTrail"
  ],
  "detail": {
    "eventSource": [
      "logs.amazonaws.com"
    ],
    "eventName": [
      "CreateLogGroup"
    ]
  }
}
PATTERN

  tags = merge({ "description" = "Log Group Creation CloudWatch Rule" }, var.tags)
}

resource "aws_cloudwatch_event_target" "log_group_creation" {
  rule = aws_cloudwatch_event_rule.log_group_creation.name
  arn  = aws_lambda_function.log_retention.arn
}

resource "aws_lambda_permission" "log_retention" {
  statement_id  = "AllowExecutionFromCloudWatch"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.log_retention.function_name
  principal     = "events.amazonaws.com"
  source_arn    = aws_cloudwatch_event_rule.log_group_creation.arn
}

resource "aws_lambda_function_event_invoke_config" "log_retention" {
  function_name          = aws_lambda_function.log_retention.function_name
  maximum_retry_attempts = 2 # This is default, but setting it to ensure that is the case.
}
