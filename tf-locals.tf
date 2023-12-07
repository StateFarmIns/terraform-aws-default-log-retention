terraform {
  required_providers {
    aws = {
      source = "hashicorp/aws"
      version = ">5.26.0" # 5.26.0 supports Lambda runtime provided.al2023
    }
  }
}

locals {
  log_retention_lambda_name        = "${var.name}-log-retention-setter"
  global_log_retention_lambda_name = "${var.name}-global-log-retention-setter"
  iam_role_name                    = "${local.log_retention_lambda_name}${var.iam_role_suffix}"
  log_group_tags_json              = var.log_group_tags == null ? "" : jsonencode(var.log_group_tags) # Null causes JSON parse error in Lambda

  runtime       = "provided.al2023"
  architectures = ["arm64"]
}
