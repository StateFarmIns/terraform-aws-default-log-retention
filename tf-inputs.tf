variable "name" {
  type        = string
  description = "Base name for all resources. E.x. <short product name>."
}

variable "subnet_ids" {
  type        = list(string)
  description = "If using a VPC, provide the IDs of the subnets you would like to deploy the Lambda to."
  default     = null
}

variable "https_egress_security_group_id" {
  type        = string
  description = "If using a VPC, pass the ID of a security group which provides egress on port 443 to CloudWatch Logs."
  default     = null
}

variable "kms_key_arn" {
  type        = string
  default     = null
  description = "If using a KMS key, provide it."
}

variable "permissions_boundary_arn" {
  type        = string
  default     = null
  description = "Provide a permissions boundary ARN if you are bound by one."
}

variable "alarm_configuration" {
  type        = any
  description = "Provide either `sns_topic_arn` to an existing SNS topic, or a list of email users `email_notification_list` to subscribe for notifications. Alarm creation is REQUIRED for this module. Note that retention setting is retried automatically, so an alarm may mean that it failed the first time and succeeded the second time. Investigating logs for each failure is recommended."

  validation {
    condition = (
      can([var.alarm_configuration.sns_topic_arn])
      ||
      can([var.alarm_configuration.email_notification_list])
    )
    error_message = "Must pass either a SNS topic ARN or an email notification list."
  }
}

variable "log_level" {
  type        = string
  default     = "info"
  description = "Override Lambda log level (trace/debug/info/warn/error)"
}

variable "log_retention_in_days" {
  type        = number
  default     = 90
  description = "Default number of days to set on new log groups. Must be a valid option that CloudWatch Logs support: https://docs.aws.amazon.com/AmazonCloudWatchLogs/latest/APIReference/API_PutRetentionPolicy.html#API_PutRetentionPolicy_RequestParameters"
}

variable "log_group_tags" {
  type        = map(string)
  default     = null
  description = "Set of tags to put on all log groups when retention is set. If not set, no tags will be added. If set, a `retention` tag will automatically be added to this list."
}

variable "set_on_all_existing_groups" {
  type        = bool
  default     = true
  description = "Set to false to disable running a bit of code which will set retention on all existing groups."
}

variable "global_log_retention_run_period" {
  type        = number
  default     = 60 * 6
  description = "Set to a number of minutes to invoke the global log retention Lambda on a schedule. Note that running it may cause perpetual diffs in other people's Terraform if they are creating a log group and not setting retention."
}

variable "metric_namespace" {
  type        = string
  default     = "LogRetention"
  description = "CloudWatch Metric namespace for custom metrics emitted by these Lambdas."
}

variable "iam_role_suffix" {
  type        = string
  default     = ""
  description = "Due to Terraform limitations, this module always creates an IAM role. Pass in a suffix for the IAM role name so that it does not conflict between regions."
}

variable "tags" {
  type        = map(string)
  default     = null
  description = "Adds tags to all created resources. It is highly recommended to use the AWS Provider's default tags instead of this variable. See: https://www.hashicorp.com/blog/default-tags-in-the-terraform-aws-provider. You can also use this input to add additional tags above and beyond the tags that are added by default_tags."
}
