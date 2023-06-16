use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CloudTrailEvent {
    pub detail: CloudTrailEventDetail,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CloudTrailEventDetail {
    pub aws_region: String,
    pub user_identity: CloudTrailEventUserIdentity,
    pub request_parameters: CloudTrailEventRequestParameters,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CloudTrailEventRequestParameters {
    pub log_group_name: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CloudTrailEventUserIdentity {
    pub account_id: String,
}

impl CloudTrailEvent {
    pub fn new(account_id: impl Into<String>, aws_region: impl Into<String>, log_group_name: impl Into<String>) -> Self {
        Self {
            detail: CloudTrailEventDetail {
                request_parameters: CloudTrailEventRequestParameters {
                    log_group_name: log_group_name.into(),
                },
                aws_region: aws_region.into(),
                user_identity: CloudTrailEventUserIdentity { account_id: account_id.into() },
            },
        }
    }
}
