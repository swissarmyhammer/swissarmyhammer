# System Administration Examples

This guide provides practical examples for using the shell tool in system administration tasks, including monitoring, maintenance, troubleshooting, and automation.

## System Monitoring

### Resource Monitoring

**Basic system resource checks**:
```bash
# CPU usage and load average
sah shell "uptime && top -b -n 1 | head -20"

# Memory usage detailed breakdown
sah shell "free -h && cat /proc/meminfo | head -10"

# Disk usage by filesystem
sah shell "df -h && lsblk"

# Network interface statistics
sah shell "ip addr show && ss -tuln"

# System temperature (if sensors available)
sah shell "sensors 2>/dev/null || echo 'lm-sensors not available'"
```

**Advanced resource monitoring**:
```bash
# Detailed CPU information
sah shell "lscpu && cat /proc/cpuinfo | grep 'model name' | head -1"

# Memory usage by process
sah shell "ps aux --sort=-%mem | head -20"

# Disk I/O statistics  
sah shell "iostat -x 1 3 2>/dev/null || vmstat 1 3"

# Network traffic statistics
sah shell "cat /proc/net/dev | grep -E '(eth|wlan|en)'"
```

### Process Management

**Process monitoring and control**:
```bash
# Show top CPU consuming processes
sah shell "ps aux --sort=-%cpu | head -20"

# Find processes by name
sah shell "pgrep -f nginx && ps -fp \$(pgrep -f nginx)"

# Kill processes gracefully
sah shell "pkill -TERM -f 'process-name'"

# Force kill if necessary (be careful!)
sah shell -t 30 "pkill -KILL -f 'stuck-process' && sleep 5"

# Check process tree
sah shell "pstree -p \$(pgrep -f myapp)"
```

**Service management**:
```bash
# Check service status
sah shell "systemctl status nginx"

# Start/stop/restart services
sah shell "systemctl restart nginx"
sah shell "systemctl stop problematic-service"
sah shell "systemctl start critical-service"

# Enable/disable services
sah shell "systemctl enable myapp"
sah shell "systemctl disable old-service"

# Check service logs
sah shell "journalctl -u nginx --since '1 hour ago' --no-pager"
```

## Log Analysis and Management

### Log File Monitoring

**Real-time log monitoring**:
```bash
# Tail multiple log files
sah shell -t 300 "tail -f /var/log/syslog /var/log/auth.log"

# Monitor application logs with filtering
sah shell -t 300 "tail -f /var/log/nginx/access.log | grep -E '(4[0-9]{2}|5[0-9]{2})'"

# Monitor system messages
sah shell -t 180 "journalctl -f --since '10 minutes ago'"
```

**Log analysis and searching**:
```bash
# Search for errors in logs
sah shell "grep -i error /var/log/syslog | tail -20"

# Analyze Apache/Nginx access patterns
sah shell "awk '{print \$1}' /var/log/nginx/access.log | sort | uniq -c | sort -nr | head -20"

# Check authentication failures
sah shell "grep 'Failed password' /var/log/auth.log | tail -10"

# Analyze disk space usage in logs
sah shell "du -sh /var/log/* | sort -rh | head -10"
```

**Log rotation and cleanup**:
```bash
# Compress old log files
sah shell -t 600 "find /var/log -name '*.log' -mtime +7 -exec gzip {} \;"

# Clean up old compressed logs  
sah shell "find /var/log -name '*.gz' -mtime +30 -delete"

# Rotate application logs
sah shell "logrotate -f /etc/logrotate.d/myapp"

# Check log rotation status
sah shell "cat /var/lib/logrotate/status"
```

## System Maintenance

### Package Management

**System updates (Ubuntu/Debian)**:
```bash
# Update package lists
sah shell -t 300 "apt update"

# Show available upgrades
sah shell "apt list --upgradable"

# Upgrade system packages
sah shell -t 1800 "DEBIAN_FRONTEND=noninteractive apt upgrade -y"

# Clean package cache
sah shell "apt autoremove -y && apt autoclean"

# Check for security updates
sah shell "apt list --upgradable | grep -i security"
```

**Package management (RHEL/CentOS)**:
```bash
# Update system packages
sah shell -t 1800 "yum update -y"

# Install specific packages
sah shell -t 600 "yum install -y htop vim-enhanced"

# Search for packages
sah shell "yum search nginx"

# Clean package cache
sah shell "yum clean all"

# Check installed packages
sah shell "rpm -qa | grep -i python | head -10"
```

### File System Maintenance

**Disk space management**:
```bash
# Find large files and directories
sah shell -t 300 "find / -type f -size +100M 2>/dev/null | head -20"

# Directory size analysis
sah shell -t 180 "du -sh /var/* | sort -rh | head -20"

# Find old files for cleanup
sah shell "find /tmp -type f -mtime +7 -ls"

# Clean temporary files
sah shell "find /tmp -type f -mtime +1 -delete"
sah shell "find /var/tmp -type f -mtime +7 -delete"
```

**File system checks**:
```bash
# Check filesystem usage and inodes
sah shell "df -h && df -i"

# Check for filesystem errors (unmounted filesystems)
sah shell -t 1800 "fsck -n /dev/sdb1"

# Check mount points
sah shell "mount | grep -E '^/dev'"

# Display filesystem information
sah shell "lsblk -f"
```

### Backup Operations

**Database backups**:
```bash
# PostgreSQL backup
sah shell -t 3600 -e "PGPASSWORD=secretpass" \
  "pg_dump -h localhost -U dbuser mydb > /backup/mydb_\$(date +%Y%m%d).sql"

# MySQL backup
sah shell -t 3600 "mysqldump -u root -p'password' mydatabase > /backup/mydatabase_\$(date +%Y%m%d).sql"

# Compress database backups
sah shell -t 1800 "gzip /backup/*.sql"

# Clean old database backups
sah shell "find /backup -name '*.sql.gz' -mtime +30 -delete"
```

**File system backups**:
```bash
# Rsync backup to remote server
sah shell -t 7200 "rsync -avz --delete /important/data/ backup@server:/backups/data/"

# Create tarball backup
sah shell -t 3600 "tar czf /backup/system-\$(date +%Y%m%d).tar.gz \
  --exclude=/proc --exclude=/sys --exclude=/dev /etc /var/log /home"

# Verify backup integrity
sah shell -t 300 "tar tzf /backup/system-\$(date +%Y%m%d).tar.gz > /dev/null"
```

## Security Administration

### Security Monitoring

**System security checks**:
```bash
# Check for failed login attempts
sah shell "grep 'Failed password' /var/log/auth.log | tail -20"

# Monitor su/sudo usage
sah shell "grep -E '(sudo|su\[)' /var/log/auth.log | tail -10"

# Check for unusual network connections
sah shell "ss -tuln | grep -E ':(22|80|443|3306|5432)'"

# Review system changes
sah shell "find /etc -type f -mtime -1 -ls"
```

**User and permission auditing**:
```bash
# List users with shell access
sah shell "grep -E '/bin/(ba)?sh$' /etc/passwd"

# Check for users with empty passwords
sah shell "awk -F: '(\$2 == \"\") {print}' /etc/shadow"

# Find SUID/SGID files
sah shell -t 600 "find / -type f \\( -perm -4000 -o -perm -2000 \\) -ls 2>/dev/null"

# Check file permissions on sensitive files
sah shell "ls -la /etc/passwd /etc/shadow /etc/sudoers"
```

**Firewall and network security**:
```bash
# Check iptables rules
sah shell "iptables -L -n -v"

# UFW status (Ubuntu)
sah shell "ufw status verbose"

# Check listening ports
sah shell "ss -tuln | sort"

# Network connection monitoring
sah shell "ss -tuln | awk 'NR>1 {print \$5}' | cut -d: -f1 | sort | uniq -c | sort -nr"
```

### Certificate Management

**SSL/TLS certificate monitoring**:
```bash
# Check certificate expiration
sah shell -t 30 "echo | openssl s_client -connect example.com:443 2>/dev/null | \
  openssl x509 -noout -dates"

# Check certificate details
sah shell -t 30 "openssl x509 -in /etc/ssl/certs/server.crt -text -noout | \
  grep -E '(Subject:|Issuer:|Not Before:|Not After:)'"

# Verify certificate against private key
sah shell "openssl x509 -noout -modulus -in server.crt | openssl md5 && \
  openssl rsa -noout -modulus -in server.key | openssl md5"

# Check certificate chain
sah shell -t 30 "openssl s_client -connect example.com:443 -showcerts </dev/null"
```

## Network Administration

### Network Diagnostics

**Connectivity testing**:
```bash
# Basic connectivity tests
sah shell -t 60 "ping -c 5 8.8.8.8"

# DNS resolution testing
sah shell -t 30 "nslookup google.com 8.8.8.8"

# Port connectivity testing
sah shell -t 30 "nc -zv google.com 80"

# Traceroute to destination
sah shell -t 120 "traceroute google.com"
```

**Network performance testing**:
```bash
# Bandwidth testing with iperf3 (if available)
sah shell -t 60 "iperf3 -c iperf.example.com -t 30"

# Network interface statistics
sah shell "cat /proc/net/dev | grep -E '(eth|wlan|en)' | \
  awk '{print \$1, \$2, \$10}'"

# Check network latency
sah shell -t 60 "ping -c 20 8.8.8.8 | tail -1"
```

### Network Configuration

**Interface management**:
```bash
# Show network interface configuration
sah shell "ip addr show"

# Bring interface up/down
sah shell "ip link set dev eth1 up"
sah shell "ip link set dev eth1 down"

# Add/remove IP addresses
sah shell "ip addr add 192.168.1.100/24 dev eth0"
sah shell "ip addr del 192.168.1.100/24 dev eth0"

# Show routing table
sah shell "ip route show"
```

## Automation Scripts

### System Health Check Script

**Comprehensive system health check**:
```bash
#!/bin/bash
# system-health-check.sh

LOGFILE="/var/log/system-health-$(date +%Y%m%d-%H%M%S).log"

log() {
    echo "$(date '+%Y-%m-%d %H:%M:%S') - $1" | tee -a "$LOGFILE"
}

log "Starting system health check..."

# CPU usage check
CPU_USAGE=$(sah shell "top -bn1 | grep 'Cpu(s)' | awk '{print \$2}'" | cut -d'%' -f1)
if [ "$(echo "$CPU_USAGE > 80" | bc)" -eq 1 ]; then
    log "WARNING: High CPU usage: ${CPU_USAGE}%"
else
    log "CPU usage normal: ${CPU_USAGE}%"
fi

# Memory usage check  
MEMORY_USAGE=$(sah shell "free | grep Mem | awk '{printf \"%.1f\", \$3/\$2 * 100.0}'")
if [ "$(echo "$MEMORY_USAGE > 90" | bc)" -eq 1 ]; then
    log "WARNING: High memory usage: ${MEMORY_USAGE}%"
else
    log "Memory usage normal: ${MEMORY_USAGE}%"
fi

# Disk space check
while IFS= read -r line; do
    usage=$(echo "$line" | awk '{print $5}' | sed 's/%//')
    mount=$(echo "$line" | awk '{print $6}')
    
    if [ "$usage" -gt 90 ]; then
        log "CRITICAL: Disk usage high on $mount: ${usage}%"
    elif [ "$usage" -gt 80 ]; then
        log "WARNING: Disk usage elevated on $mount: ${usage}%"
    else
        log "Disk usage normal on $mount: ${usage}%"
    fi
done < <(sah shell "df -h | grep -E '^/dev'")

# Service status check
services=("nginx" "ssh" "rsyslog")
for service in "${services[@]}"; do
    if sah shell --quiet "systemctl is-active $service"; then
        log "Service $service is running"
    else
        log "CRITICAL: Service $service is not running"
    fi
done

# Network connectivity check
if sah shell --quiet -t 30 "ping -c 3 8.8.8.8"; then
    log "Network connectivity OK"
else
    log "WARNING: Network connectivity issues detected"
fi

log "System health check completed. Results in $LOGFILE"
```

### Automated Maintenance Script

**Daily maintenance automation**:
```bash
#!/bin/bash
# daily-maintenance.sh

# Run as root or with sudo
if [ "$EUID" -ne 0 ]; then
    echo "Please run as root or with sudo"
    exit 1
fi

MAINTENANCE_LOG="/var/log/daily-maintenance-$(date +%Y%m%d).log"

log() {
    echo "$(date '+%Y-%m-%d %H:%M:%S') - $1" | tee -a "$MAINTENANCE_LOG"
}

log "Starting daily maintenance routine..."

# Update package lists
log "Updating package lists..."
sah shell -t 300 "apt update" >> "$MAINTENANCE_LOG" 2>&1

# Clean temporary files
log "Cleaning temporary files..."
sah shell "find /tmp -type f -mtime +1 -delete" >> "$MAINTENANCE_LOG" 2>&1
sah shell "find /var/tmp -type f -mtime +7 -delete" >> "$MAINTENANCE_LOG" 2>&1

# Rotate logs
log "Rotating logs..."
sah shell "logrotate -f /etc/logrotate.conf" >> "$MAINTENANCE_LOG" 2>&1

# Clean package cache
log "Cleaning package cache..."
sah shell "apt autoremove -y && apt autoclean" >> "$MAINTENANCE_LOG" 2>&1

# Check disk space
log "Checking disk space..."
sah shell "df -h" >> "$MAINTENANCE_LOG" 2>&1

# Backup critical configuration files
log "Backing up configuration files..."
sah shell -t 300 "tar czf /backup/config-backup-$(date +%Y%m%d).tar.gz \
    /etc/nginx /etc/apache2 /etc/mysql /etc/postgresql /etc/ssh" >> "$MAINTENANCE_LOG" 2>&1

# Clean old backups (keep 30 days)
log "Cleaning old backups..."
sah shell "find /backup -name '*.tar.gz' -mtime +30 -delete" >> "$MAINTENANCE_LOG" 2>&1

# System security check
log "Running security checks..."
sah shell "lynis audit system --quick" >> "$MAINTENANCE_LOG" 2>&1 || log "Lynis not available, skipping security audit"

log "Daily maintenance completed successfully"

# Email report if mail is configured
if command -v mail >/dev/null; then
    cat "$MAINTENANCE_LOG" | mail -s "Daily Maintenance Report - $(hostname)" admin@example.com
fi
```

## Monitoring and Alerting

### Resource Usage Monitoring

**Create monitoring scripts with thresholds**:
```bash
#!/bin/bash
# resource-monitor.sh

ALERT_EMAIL="admin@example.com"
HOSTNAME=$(hostname)

# CPU threshold (percentage)
CPU_THRESHOLD=85

# Memory threshold (percentage)  
MEMORY_THRESHOLD=90

# Disk threshold (percentage)
DISK_THRESHOLD=85

check_cpu() {
    cpu_usage=$(sah shell "top -bn1 | grep 'Cpu(s)' | awk '{print \$2}'" | cut -d'%' -f1 | cut -d',' -f1)
    
    if [ "$(echo "$cpu_usage > $CPU_THRESHOLD" | bc)" -eq 1 ]; then
        echo "ALERT: High CPU usage on $HOSTNAME: ${cpu_usage}%" | \
            mail -s "CPU Alert - $HOSTNAME" "$ALERT_EMAIL"
    fi
}

check_memory() {
    memory_usage=$(sah shell "free | grep Mem | awk '{printf \"%.0f\", \$3/\$2 * 100.0}'")
    
    if [ "$memory_usage" -gt "$MEMORY_THRESHOLD" ]; then
        echo "ALERT: High memory usage on $HOSTNAME: ${memory_usage}%" | \
            mail -s "Memory Alert - $HOSTNAME" "$ALERT_EMAIL"
    fi
}

check_disk() {
    while IFS= read -r line; do
        usage=$(echo "$line" | awk '{print $5}' | sed 's/%//')
        mount=$(echo "$line" | awk '{print $6}')
        
        if [ "$usage" -gt "$DISK_THRESHOLD" ]; then
            echo "ALERT: High disk usage on $HOSTNAME $mount: ${usage}%" | \
                mail -s "Disk Alert - $HOSTNAME" "$ALERT_EMAIL"
        fi
    done < <(sah shell "df -h | grep -E '^/dev'")
}

# Run checks
check_cpu
check_memory  
check_disk
```

### Log Monitoring and Alerting

**Monitor logs for specific patterns**:
```bash
#!/bin/bash
# log-monitor.sh

LOGFILE="/var/log/syslog"
ERROR_PATTERNS=("kernel.*error" "Out of memory" "segfault" "failed.*authentication")
ALERT_EMAIL="admin@example.com"
HOSTNAME=$(hostname)

# Check for error patterns in logs from last hour
for pattern in "${ERROR_PATTERNS[@]}"; do
    count=$(sah shell "grep -c -i '$pattern' <(journalctl --since '1 hour ago' --no-pager)" || echo "0")
    
    if [ "$count" -gt 0 ]; then
        {
            echo "Alert: Found $count instances of pattern '$pattern' in system logs on $HOSTNAME"
            echo "Time: $(date)"
            echo "Recent occurrences:"
            sah shell "journalctl --since '1 hour ago' --no-pager | grep -i '$pattern' | tail -5"
        } | mail -s "Log Alert - $pattern - $HOSTNAME" "$ALERT_EMAIL"
    fi
done
```

This system administration guide provides comprehensive examples for monitoring, maintenance, security, and automation tasks. Adapt the scripts and commands based on your specific system requirements and security policies.