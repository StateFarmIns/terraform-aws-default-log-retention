use aws_sdk_cloudwatch::types::MetricDatum;
use log::warn;

use crate::{cloudwatch_metrics_traits::PutMetricData, global::metric_namespace};

// Enumerates the titles of metric names
// to ensure consistency between Lambdas
#[derive(Debug, Clone)]
pub enum MetricName {
    Total,
    Updated,
    AlreadyHasRetention,
    AlreadyTaggedWithRetention,
    Errored,
}

#[derive(Debug, Clone)]
pub struct Metric {
    pub name: MetricName,
    pub value: f64,
}

impl From<Metric> for MetricDatum {
    fn from(metric: Metric) -> Self {
        MetricDatum::builder().metric_name(metric.name.to_string()).value(metric.value).build()
    }
}

impl Metric {
    pub fn new(name: MetricName, value: f64) -> Self {
        Self { name, value }
    }
}

impl std::fmt::Display for MetricName {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub async fn publish_metrics(client: impl PutMetricData, metrics: Vec<Metric>) {
    let metrics = metrics.iter().map(|metric| metric.clone().into()).collect();

    let result = client.put_metric_data(metric_namespace(), metrics).await;

    if let Err(error) = result {
        warn!("Metric publish failed. Error: {:?}", error)
    }
}

pub async fn publish_metric(client: impl PutMetricData, metric: Metric) {
    let metrics = vec![metric];
    publish_metrics(client, metrics).await
}

#[cfg(test)]
mod tests {
    

    use super::*;
    use async_trait::async_trait;
    use aws_sdk_cloudwatch::{operation::put_metric_data::PutMetricDataOutput, types::error::InternalServiceFault, Error as CloudWatchError};
    
    use mockall::mock;

    #[test]
    fn test_metric_into_metric_datum() {
        let metric = Metric::new(MetricName::Total, 75.0);
        let actual: MetricDatum = metric.into();

        let expected = MetricDatum::builder().metric_name("Total").value(75.0).build();

        assert_eq!(expected, actual);
    }

    #[tokio::test]
    async fn test_publish_metrics_success() {
        let mut cw_metrics_mock = MockCloudWatchMetrics::new();
        cw_metrics_mock
            .expect_put_metric_data()
            .once()
            .withf(|namespace, metrics| {
                assert_eq!("LogRotation", namespace);
                insta::assert_debug_snapshot!("CWMetricCall_publish_metrics_success", metrics);
                true
            })
            .returning(|_, _| Ok(PutMetricDataOutput::builder().build()));

        let metrics = vec![Metric::new(MetricName::Updated, 7.0), Metric::new(MetricName::Total, 9.0)];

        publish_metrics(cw_metrics_mock, metrics).await;
    }

    #[tokio::test]
    async fn test_publish_metrics_failed() {
        let mut cw_metrics_mock = MockCloudWatchMetrics::new();
        cw_metrics_mock
            .expect_put_metric_data()
            .once()
            .withf(|namespace, metrics| {
                assert_eq!("LogRotation", namespace);
                insta::assert_debug_snapshot!("CWMetricCall_publish_metrics_failed", metrics);
                true
            })
            .returning(|_, _| Err(CloudWatchError::InternalServiceFault(InternalServiceFault::builder().build())));

        let metrics = vec![Metric::new(MetricName::Updated, 7.0), Metric::new(MetricName::Total, 9.0)];

        publish_metrics(cw_metrics_mock, metrics).await;
    }

    mock! {
        pub CloudWatchMetrics {}

        #[async_trait]
        impl PutMetricData for CloudWatchMetrics {
            async fn put_metric_data(
                &self,
                namespace: String,
                metric_data: Vec<MetricDatum>,
            ) -> Result<PutMetricDataOutput, CloudWatchError>;
        }
    }
}
