data "aws_region" "current" {}
data "aws_iam_account_alias" "current" {}
data "aws_caller_identity" "current" {}

# .issuer_arn grabs the underlying ARN (removes the assumed-role portion)
data "aws_iam_session_context" "current" {
  arn = data.aws_caller_identity.current.arn
}

