# Email Server

A simple SMTP email server built with Rust.

## Quick Start

1. Put `email_config.json` and `email-server` in the same folder.
2. Edit `email_config.json` with your settings:
    - smtp\_server: Your SMTP server address
    - smtp\_port: SMTP server port
    - email\_account: Your email account
    - email\_password: Your email password
    - email\_from: Default sender email
    - email\_to: Default recipient email
    - sender\_name: Default sender display name
    - api\_key: API key for authentication
3. run `./email-server`

## API Usage

Send an email:

```bash
curl -X POST \
  http://localhost:3000/send-email \
  -H 'Content-Type: application/json' \
  -H 'X-API-Key: your-api-key' \
  -d '{
    "from": "your-email@example.com",
    "to": "recipient@example.com",
    "sender_name": "Custom Name",
    "subject": "Test Email",
    "body": "Hello!"
}'
```

`from` , `to` and `sender_name` are optional, set it to empty to use the defaults in `email_config.json` :

```bash
curl -X POST \
  http://localhost:3000/send-email \
  -H 'Content-Type: application/json' \
  -H 'X-API-Key: your-api-key' \
  -d '{
    "subject": "Test Email",
    "body": "Hello!"
}'
```
