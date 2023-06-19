resource "aws_cloudwatch_log_group" "log_retention_lambda" {
  name              = "/aws/lambda/${local.log_retention_lambda_name}"
  retention_in_days = 30
  tags              = var.tags
}

resource "aws_cloudwatch_log_group" "global_log_retention_lambda" {
  name              = "/aws/lambda/${local.global_log_retention_lambda_name}"
  retention_in_days = 30
  tags              = var.tags
}
