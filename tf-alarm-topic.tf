locals {
  create_sns_topic = can([var.alarm_configuration.email_notification_list])
  sns_topic_arn    = local.create_sns_topic ? aws_sns_topic.alarms[0].arn : var.alarm_configuration.sns_topic_arn
}

resource "aws_sns_topic" "alarms" {
  count = local.create_sns_topic ? 1 : 0
  name  = var.name

  kms_master_key_id = data.aws_kms_key.master.id
  tags              = var.tags
}

resource "aws_sns_topic_subscription" "alarms" {
  for_each  = local.create_sns_topic ? toset(var.alarm_configuration.email_notification_list) : toset([])
  topic_arn = aws_sns_topic.alarms[0].arn
  protocol  = "email"
  endpoint  = each.value
}

resource "aws_sns_topic_policy" "alarms" {
  count  = local.create_sns_topic ? 1 : 0
  arn    = aws_sns_topic.alarms[count.index].arn
  policy = data.aws_iam_policy_document.alarms[count.index].json
}

data "aws_iam_policy_document" "alarms" {
  count = local.create_sns_topic ? 1 : 0
  statement {
    sid = "CloudWatchAlarm"
    principals {
      type        = "Service"
      identifiers = ["cloudwatch.amazonaws.com"]
    }
    effect    = "Allow"
    actions   = ["sns:Publish"]
    resources = [aws_sns_topic.alarms[count.index].arn]
    condition {
      test     = "StringEquals"
      variable = "aws:SourceAccount"
      values   = [data.aws_caller_identity.current.account_id]
    }
  }

  statement {
    sid = "Owners"
    principals {
      type        = "AWS"
      identifiers = [data.aws_iam_session_context.current.issuer_arn]
    }
    effect = "Allow"
    # cannot do sns:* for SNS access policy
    # https://docs.aws.amazon.com/sns/latest/dg/sns-access-policy-language-api-permissions-reference.html
    actions = [
      "sns:GetTopicAttributes",
      "sns:SetTopicAttributes",
      "sns:AddPermission",
      "sns:RemovePermission",
      "sns:DeleteTopic",
      "sns:Subscribe",
      "sns:ListSubscriptionsByTopic",
      "sns:Publish"
    ]
    resources = [aws_sns_topic.alarms[count.index].arn]
  }
}
