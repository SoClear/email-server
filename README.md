# Email Server

A simple SMTP email server built with Rust.

## Quick Start

1. Put `app_config.json` and `email-server` in the same folder.
2. Edit `app_config.json` with your settings:

    ```json
    {
        "email": {
            "smtp_server": "smtp.example.com",
            "smtp_port": 587,
            "email_account": "your-email@example.com",
            "email_password": "your-password",
            "email_from": "your-email@example.com",
            "email_to": "default-to@example.com",
            "sender_name": "default sender name"
        },
        "server": {
            "api_key": "your-api-key",
            "server_host": "0.0.0.0",
            "server_port": 3000
        }

    }
    ```

    - smtp\_server: Your SMTP server address
    - smtp\_port: SMTP server port
    - email\_account: Your email account
    - email\_password: Your email password
    - email\_from: Default sender email
    - email\_to: Default recipient email
    - sender\_name: Default sender display name
    - api\_key: API key for authentication
    - server\_host: Server host address, optional, default is `0.0.0.0`
    - server\_port: Server port, optional, default is `3000`

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

`from` , `to` and `sender_name` are optional, set it to empty to use the defaults in `app_config.json` :

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
