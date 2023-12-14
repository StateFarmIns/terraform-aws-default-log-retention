resource "aws_iam_role" "log_retention" {
  name                 = local.iam_role_name
  assume_role_policy   = data.aws_iam_policy_document.lambda_assume.json
  permissions_boundary = var.permissions_boundary_arn

  inline_policy {
    name   = "log_retention"
    policy = data.aws_iam_policy_document.log_retention.json
  }

  tags = var.tags
}

data "aws_iam_policy_document" "log_retention" {
  statement {
    actions = [
      "logs:CreateLogGroup",
      "logs:CreateLogStream",
      "logs:PutLogEvents",
      "logs:ListTagsForResource",
      "logs:TagResource",
      "logs:PutRetentionPolicy",
      "logs:DescribeLogGroups"
    ]
    resources = ["arn:${data.aws_partition.current.partition}:logs:*:*:*"]
  }

  statement {
    actions   = ["tag:GetResources"]
    resources = ["*"]
  }

  statement {
    actions   = ["cloudwatch:PutMetricData"]
    resources = ["*"]
    condition {
      test     = "StringEquals"
      variable = "cloudwatch:namespace"
      values   = [var.metric_namespace]
    }
  }

  dynamic "statement" {
    for_each = var.kms_key_arn == null ? [] : [var.kms_key_arn]
    content {
      actions   = ["kms:Decrypt"]
      resources = [statement.value]
    }
  }

  statement {
    actions   = ["ec2:*NetworkInterface*"]
    resources = ["*"]
  }
}

data "aws_iam_policy_document" "lambda_assume" {
  statement {
    actions = ["sts:AssumeRole"]

    principals {
      type        = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }
  }
}
