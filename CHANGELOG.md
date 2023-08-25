# 1.0.0 (2023-08-25)


### Bug Fixes

* 🐛 Add conditional for dynamic KMS permission ([4c939e5](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/4c939e5841f872380310a9c9969a3a01a09f1583))
* 🐛 Allow null in alarm config ([e2d4a97](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/e2d4a97a2029781e5a10e4092e399737661710b6))
* 🐛 fix ternary ([e0f2f45](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/e0f2f45d4c5c92f9aacb1ab80a76797709f988fc))
* 🐛 Fix vulnerability ([9526304](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/952630440e56455d640d0f913e586233bc2ea6dc))
* 🐛 Fixed inconsistent type in ternary statement ({} vs []) ([55920cf](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/55920cf288110b993158a5448c886c3846bdfc61))
* 🐛 Remove State Farm specific KMS key logic ([64dc3b0](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/64dc3b0052b4f197cce80bb254c8ce29ab29698a))
* 🐛 Remove State Farm specific Lambda reference ([445c260](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/445c260c9ecf482aca8a0740fe9e41ec2f55530d))
* **common:** adding cargo-zigbuild to try and fix pipeline ([20af81b](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/20af81bd9d2231c5c38b06375788decc9fdc30b3))
* **common:** adding release step to audit job for pull requests ([5fde8ce](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/5fde8ce01f4ad25253f0ae586d427a5571451433))
* **common:** changing indent in .releaserc.yml to fix error ([741c899](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/741c899cdb6c7ebd4e68f72dfc263609c57b1c91))
* **common:** cleaning up test components ([5e78b61](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/5e78b61d6d183e2204e001b37c841e1a869771a0))
* **common:** fixing yml syntax errors in workflow ([11101e4](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/11101e4c7add82d9153f995d887da9b54f644a0a))
* **common:** installing semantic-release plugin via npm ([a737908](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/a7379087a046fc41c44b0c3bebaaeae228d2ec2f))
* **common:** moving cargo-zigbuild to build job ([48ef5b8](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/48ef5b8546096ef6468ddbbd8b066363db0e6e9a))
* **common:** referencing upload-artifact action in workflow ([fe6858a](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/fe6858a3df146b9443a5577907551cf66af1f614))
* **common:** removing codeql due to lack of rust support ([a79f3e4](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/a79f3e422d8b15289c9055ea98c12e9f5dacfff8))
* **common:** removing zigbuild ([89148fc](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/89148fc342d2050df98602e30fee968c0c968f86))
* **common:** removing zigbuild to see if pipeline breaks ([0eb25a1](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/0eb25a125931a32b0b6c53e3ff9806b63d8adf67))
* **common:** specifying version for semver install ([ffd1329](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/ffd13291926c963a94f9c2aac37ff426681e3c12))
* **common:** trying alternative install of semantic release plugin ([31f49a6](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/31f49a6357dad40bb3fca4f3ed9f6e876c8b0727))


### Features

* 🎸 Allow user to opt out of alarm creation ([83d3419](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/83d3419b2a27fe60608825a426ef76ae3df9bf5e))
* 🎸 Initial commit of existing code and Terraform ([61a1fb4](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/61a1fb4810178d1e799358ba8fe6788608a71e3d))
* 🎸 Rework VPC/Security Group to not be SF-specific ([0ff5431](https://github.com/StateFarmIns/terraform-aws-default-log-retention/commit/0ff5431fcc7f2cb4b19adf97bfc2a6b432a15802))
