# Alertmanager Slack Configuration

## Security Notice
The Slack webhook URL should NEVER be committed to the repository.

## Setup Instructions

1. Create a file containing your Slack webhook URL:
   ```bash
   echo "https://hooks.slack.com/services/YOUR/WEBHOOK/URL" > /etc/alertmanager/slack_webhook_url
   chmod 600 /etc/alertmanager/slack_webhook_url
   ```

2. The alertmanager.yml is configured to read from this file using `api_url_file`

## Docker Compose Setup
If using docker-compose, mount the webhook file:

```yaml
services:
  alertmanager:
    volumes:
      - ./alertmanager.yml:/etc/alertmanager/config.yml
      - /path/to/your/slack_webhook_url:/etc/alertmanager/slack_webhook_url:ro
```

## Getting a Slack Webhook URL
1. Go to https://api.slack.com/apps
2. Create a new app or select existing
3. Enable "Incoming Webhooks"
4. Add a new webhook to your workspace
5. Copy the webhook URL