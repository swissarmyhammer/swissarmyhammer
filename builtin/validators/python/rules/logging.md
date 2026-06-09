---
name: logging
description: Structured logging only, log to stdout, JSON in production
severity: warn
---

# Python Logging

- **Structured logging only.** `logger.info(f"Order {order_id} processed")` cannot be indexed or queried. Use `structlog` or equivalent: `logger.info("order.processed", order_id=order_id)`.
- **Log to stdout.** Let infrastructure (systemd, Docker, Kubernetes) handle routing. Applications should not configure log files or rotation.
- **JSON in production, pretty-print in development.** A hard-coded log format is a finding.
