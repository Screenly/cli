## Monitoring

*Monitoring is in invite-only beta.*

When building Edge Apps, you sometimes need the ability to monitor performance of the device running the Edge App. This is particularly true for low-powered devices, like Raspberry Pis where you have very limited resources to work with.

To help you with this, we've decided to adopt [Prometheus](https://prometheus.io), as the platform to expose metrics.

With the monitoring feature enabled, the device will the Prometheus end-point on port `9100`, where we expose data from [Node Exporter](https://prometheus.io/docs/guides/node-exporter/#monitoring-linux-host-metrics-with-the-node-exporter). This allows you to scrape metrics and visualize them with a tool like [Grafana](https://grafana.com/) (which in turn you can [visualize with Screenly](https://www.screenly.io/tutorials/grafana/)).
