use async_trait::async_trait;
use aws_sdk_cloudwatch::{error::PutMetricDataError, model::MetricDatum, output::PutMetricDataOutput, types::SdkError, Client as CloudWatchMetricsClient};

#[cfg(test)]
use mockall::automock;

/* Base Struct */

#[derive(Clone, Debug)]
pub struct CloudWatchMetrics {
    client: CloudWatchMetricsClient,
}

impl CloudWatchMetrics {
    pub fn new(client: CloudWatchMetricsClient) -> Self {
        Self { client }
    }
}

/* End Base Struct */

/* Traits */

#[cfg_attr(test, automock)]
#[async_trait]
pub trait PutMetricData {
    async fn put_metric_data(&self, namespace: String, metric_data: Vec<MetricDatum>) -> Result<PutMetricDataOutput, SdkError<PutMetricDataError>>;
}

/* End Traits */

/* Implementations */

#[async_trait]
impl PutMetricData for CloudWatchMetrics {
    async fn put_metric_data(&self, namespace: String, metric_data: Vec<MetricDatum>) -> Result<PutMetricDataOutput, SdkError<PutMetricDataError>> {
        Ok(self
            .client
            .put_metric_data()
            .set_metric_data(Some(metric_data))
            .namespace(namespace)
            .send()
            .await?)
    }
}

/* End Implementations */
