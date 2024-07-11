## Monitoring

*Monitoring is in invite-only beta.*

When developing Edge Apps, it's crucial to monitor the performance of the device running the app. This is especially important for low-powered devices, such as Raspberry Pis, where resources are limited.

To assist with monitoring, we have chosen [Prometheus](https://prometheus.io) as the platform to expose metrics.

When monitoring is enabled, the device exposes Prometheus metrics at port `9100`, utilizing [Node Exporter](https://prometheus.io/docs/guides/node-exporter/#monitoring-linux-host-metrics-with-the-node-exporter). This enables you to scrape metrics and visualize them using tools like [Grafana](https://grafana.com/), which can be integrated seamlessly with Screenly for visualization purposes ([learn more about Grafana integration with Screenly](https://www.screenly.io/tutorials/grafana/)).
