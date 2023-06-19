data "archive_file" "global_log_retention" {
  type        = "zip"
  source_file = "${path.module}/dist/global_retention_setter/bootstrap"
  output_path = "${path.module}/dist/global_retention_setter/bootstrap.zip"
}

resource "aws_lambda_function" "global_log_retention" {
  depends_on       = [aws_cloudwatch_log_group.global_log_retention_lambda]
  function_name    = local.global_log_retention_lambda_name
  filename         = data.archive_file.global_log_retention.output_path
  source_code_hash = data.archive_file.global_log_retention.output_base64sha256
  runtime          = "provided.al2"
  architectures    = ["arm64"]
  handler          = "bootstrap"
  role             = aws_iam_role.log_retention.arn
  timeout          = 900
  kms_key_arn      = var.kms_key_arn
  memory_size      = 128
  description      = "Sets default CloudWatch Logs retention settings for all existing log groups."

  environment {
    variables = {
      log_retention_in_days = var.log_retention_in_days
      log_group_tags        = local.log_group_tags_json
      metric_namespace      = var.metric_namespace
      RUST_BACKTRACE        = 1
      RUST_LOG              = "warn,global_retention_setter=${var.log_level}" # https://docs.rs/env_logger/latest/env_logger/
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

resource "aws_lambda_invocation" "run_on_existing_groups" {
  count         = var.set_on_all_existing_groups ? 1 : 0
  function_name = aws_lambda_function.global_log_retention.function_name
  input         = "{}"
}

resource "aws_lambda_function_event_invoke_config" "global_log_retention" {
  function_name          = aws_lambda_function.global_log_retention.function_name
  maximum_retry_attempts = 2 # This is default, but setting it to ensure that is the case.
}


resource "aws_cloudwatch_event_rule" "global_log_retention" {
  count               = var.global_log_retention_run_period == null ? 0 : 1
  name                = aws_lambda_function.global_log_retention.function_name
  description         = "Sets default retention for all log groups in the region every ${var.global_log_retention_run_period} minutes"
  schedule_expression = "rate(${var.global_log_retention_run_period} minutes)"
  tags                = var.tags
}

resource "aws_cloudwatch_event_target" "global_log_retention" {
  count     = var.global_log_retention_run_period == null ? 0 : 1
  rule      = aws_cloudwatch_event_rule.global_log_retention[0].name
  target_id = "lambda"
  arn       = aws_lambda_function.global_log_retention.arn
}

resource "aws_lambda_permission" "global_log_retention" {
  count         = var.global_log_retention_run_period == null ? 0 : 1
  statement_id  = "AllowExecutionFromCloudWatch"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.global_log_retention.function_name
  principal     = "events.amazonaws.com"
  source_arn    = aws_cloudwatch_event_rule.global_log_retention[0].arn
}
