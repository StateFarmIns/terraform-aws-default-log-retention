resource "aws_security_group" "https_egress" {
  name  = "${var.name}-https-egress"
  count = var.https_egress_security_group_name == null ? 1 : 0

  description = "Allows HTTPS egress for CloudWatch Logs access."
  vpc_id      = local.vpc_id

  egress {
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = merge({
    Name = "${var.name}-https-egress"
  }, var.tags)
}
