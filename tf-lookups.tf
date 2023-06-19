locals {
  vpc_id                  = var.vpc_id != null ? var.vpc_id : tolist(data.aws_vpcs.vpcs.ids)[0]
  https_security_group_id = var.https_egress_security_group_name == null ? aws_security_group.https_egress[0].id : data.aws_security_groups.https_egress[0].ids[0]
}

data "aws_vpcs" "vpcs" {}

data "aws_subnets" "subnets" {
  filter {
    name   = "vpc-id"
    values = [local.vpc_id]
  }

  tags = {
    network = "private"
    tier    = "app"
  }
}

data "aws_region" "current" {}
data "aws_iam_account_alias" "current" {}
data "aws_caller_identity" "current" {}

data "aws_security_groups" "https_egress" {
  count = var.https_egress_security_group_name == null ? 0 : 1
  filter {
    name   = "group-name"
    values = [var.https_egress_security_group_name]
  }
}

# .issuer_arn grabs the underlying ARN (removes the assumed-role portion)
data "aws_iam_session_context" "current" {
  arn = data.aws_caller_identity.current.arn
}

