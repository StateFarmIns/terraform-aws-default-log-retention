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
    pub value: u64,
}

impl Metric {
    pub fn new(name: MetricName, value: u64) -> Self {
        Self { name, value }
    }
}

impl std::fmt::Display for MetricName {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub fn publish_metrics(metrics: Vec<Metric>) {
    for metric in metrics {
        publish_metric(metric);
    }
}

pub fn publish_metric(metric: Metric) {
    metrics::absolute_counter!(metric.name.to_string(), metric.value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_metric() {
        publish_metric(Metric {
            name: MetricName::AlreadyHasRetention,
            value: 1237,
        })
    }

    #[test]
    fn test_publish_metrics() {
        let metrics = vec![
            Metric {
                name: MetricName::AlreadyHasRetention,
                value: 1,
            },
            Metric {
                name: MetricName::AlreadyTaggedWithRetention,
                value: 2,
            },
        ];
        publish_metrics(metrics);
    }
}
