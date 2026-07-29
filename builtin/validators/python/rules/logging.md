---
name: logging
description: Structured logging only, log to stdout, JSON in production
---

# Python Logging

- **Use structured logging only.** Do not write unstructured log lines like `logger.info(f"Order {order_id} processed")`. You cannot index or query them. Use `structlog` or an equivalent tool. Example: `logger.info("order.processed", order_id=order_id)`.
- **Log to stdout.** Let infrastructure such as systemd, Docker, or Kubernetes handle routing. Applications must not configure log files or rotation.
- **Use JSON in production and pretty-print in development.** A hard-coded log format is a finding.
