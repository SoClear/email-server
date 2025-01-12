use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use config::{Config, File};
use lettre::{
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
        Error as SmtpError,
    },
    Message, SmtpTransport, Transport,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
use tower_http::trace::TraceLayer;
use tracing::{debug, error, info, warn};

#[derive(Debug, Deserialize, Clone)]
struct EmailConfig {
    smtp_server: String,
    smtp_port: u16,
    email_account: String,
    email_password: String,
    email_from: String,
    email_to: String,
    sender_name: String,
}

#[derive(Debug, Deserialize, Clone)]
struct ServerConfig {
    #[serde(default = "default_server_host")] // 如果未配置，使用默认主机
    server_host: String,
    #[serde(default = "default_server_port")] // 如果未配置，使用默认端口
    server_port: u16,
    api_key: String,
}

// 默认主机函数
fn default_server_host() -> String {
    "0.0.0.0".to_string()
}

// 默认端口函数
fn default_server_port() -> u16 {
    3000
}

// 整合两个配置的结构体
#[derive(Debug, Deserialize, Clone)]
struct AppConfig {
    email: EmailConfig,
    server: ServerConfig,
}

// 请求频率限制结构
struct RateLimit {
    requests: HashMap<String, Vec<SystemTime>>,
}

impl RateLimit {
    fn new() -> Self {
        RateLimit {
            requests: HashMap::new(),
        }
    }

    fn is_allowed(&mut self, ip: &str) -> bool {
        let now = SystemTime::now();
        let requests = self.requests.entry(ip.to_string()).or_insert(Vec::new());

        requests.retain(|&time| {
            now.duration_since(time).unwrap_or(Duration::from_secs(0)) < Duration::from_secs(60)
        });

        if requests.len() >= 10 {
            warn!("Rate limit exceeded for IP: {}", ip);
            return false;
        }

        requests.push(now);
        debug!("Request allowed for IP: {} (count: {})", ip, requests.len());
        true
    }
}

// 实现错误响应转换
impl IntoResponse for EmailError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            EmailError::SmtpError(ref e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to send email: {}", e),
            ),
            EmailError::RateLimit => (
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded".to_string(),
            ),
            EmailError::InvalidApiKey => (StatusCode::UNAUTHORIZED, "Invalid API key".to_string()),
            EmailError::MissingApiKey => (StatusCode::UNAUTHORIZED, "Missing API key".to_string()),
        };

        let body = Json(ApiResponse {
            status: "error".to_string(),
            message: error_message,
        });

        (status, body).into_response()
    }
}

// 验证 API key
fn validate_api_key(headers: &HeaderMap, config_api_key: &str) -> Result<(), EmailError> {
    debug!("Checking for API key in headers...");
    let request_api_key = headers
        .get("X-API-Key")
        .ok_or_else(|| {
            warn!("No API key provided in request");
            EmailError::MissingApiKey
        })?
        .to_str()
        .map_err(|e| {
            error!("Invalid API key format: {}", e);
            EmailError::InvalidApiKey
        })?;

    if request_api_key != config_api_key {
        warn!("Invalid API key provided");
        return Err(EmailError::InvalidApiKey);
    }

    debug!("API key validation successful");
    Ok(())
}

// 发送邮件处理函数
async fn send_email(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<EmailRequest>,
) -> Result<impl IntoResponse, EmailError> {
    // 验证 API key
    validate_api_key(&headers, &state.app_config.server.api_key)?;

    // 获取客户端 IP
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    debug!("Request from IP: {}", ip);

    // 检查频率限制
    let mut rate_limit = state.rate_limit.lock().unwrap();
    if !rate_limit.is_allowed(&ip) {
        return Err(EmailError::RateLimit);
    }

    // 使用请求中的值或配置中的默认值
    let from = if req.from.is_empty() {
        debug!("Using default from address");
        &state.app_config.email.email_from
    } else {
        debug!("Using custom from address: {}", req.from);
        &req.from
    };

    let to = if req.to.is_empty() {
        debug!("Using default to address");
        &state.app_config.email.email_to
    } else {
        debug!("Using custom to address: {}", req.to);
        &req.to
    };

    info!("Preparing to send email from {} to {}", from, to);

    // 优先使用请求中的昵称，如果没有则使用配置中的昵称
    let sender_name = if !req.sender_name.is_empty() {
        debug!("Using custom sender name: {}", req.sender_name);
        &req.sender_name
    } else {
        debug!(
            "Using default sender name: {}",
            state.app_config.email.sender_name
        );
        &state.app_config.email.sender_name
    };

    // 构建发件人地址字符串，包含昵称
    let from_addr = format!("{} <{}>", sender_name, from);

    // 构建邮件
    debug!(
        "Building email message with sender name: {}",
        state.app_config.email.sender_name
    );
    let email = Message::builder()
        .from(from_addr.parse().unwrap())
        .to(to.parse().unwrap())
        .subject(req.subject)
        .body(req.body)
        .unwrap();
    debug!("Email message built successfully");

    // 发送邮件
    info!("Sending email...");
    match state.smtp_transport.send(&email) {
        Ok(_) => {
            info!("Email sent successfully to {}", to);
            Ok(Json(ApiResponse {
                status: "success".to_string(),
                message: "Email sent successfully".to_string(),
            }))
        }
        Err(e) => {
            error!("Failed to send email: {}", e);
            Err(EmailError::SmtpError(e))
        }
    }
}

// 应用状态
struct AppState {
    rate_limit: Mutex<RateLimit>,
    smtp_transport: SmtpTransport,
    app_config: AppConfig,
}

// 邮件请求结构
#[derive(Deserialize)]
struct EmailRequest {
    #[serde(default)] // 使字段成为可选
    from: String,
    #[serde(default)] // 使字段成为可选
    to: String,
    #[serde(default)] // 使字段可选
    sender_name: String, // 添加发件人昵称字段
    subject: String,
    body: String,
}

// API 响应结构
#[derive(Serialize)]
struct ApiResponse {
    status: String,
    message: String,
}

// 自定义错误类型
#[derive(thiserror::Error, Debug)]
enum EmailError {
    #[error("SMTP error: {0}")]
    SmtpError(#[from] lettre::transport::smtp::Error),
    #[error("Rate limit exceeded")]
    RateLimit,
    #[error("Invalid API key")]
    InvalidApiKey,
    #[error("Missing API key")]
    MissingApiKey,
}

// 加载配置文件
fn get_app_config() -> AppConfig {
    return Config::builder()
        .add_source(File::with_name("app_config.json"))
        .build()
        .unwrap()
        .try_deserialize()
        .unwrap();
}

// 创建 SMTP 传输
fn create_smtp_transport(email_config: &EmailConfig) -> Result<SmtpTransport, SmtpError> {
    // 创建 SMTP 凭据
    let creds = Credentials::new(
        email_config.email_account.clone(),
        email_config.email_password.clone(),
    );

    // 创建 TLS 参数
    let tls_parameters = TlsParameters::new(email_config.smtp_server.clone()).unwrap_or_else(|e| {
        error!("Failed to create TLS parameters: {}", e);
        std::process::exit(1);
    });

    // 根据 SMTP 端口选择 TLS 类型
    let tls = match email_config.smtp_port {
        465 => Tls::Wrapper(tls_parameters),
        587 => Tls::Required(tls_parameters),
        _ => Tls::Opportunistic(tls_parameters),
    };

    // 创建 SMTP 传输
    let smtp_transport = SmtpTransport::relay(&email_config.smtp_server)
        .unwrap_or_else(|e| {
            error!("Failed to create SMTP transport: {}", e);
            std::process::exit(1);
        })
        .credentials(creds)
        .port(email_config.smtp_port)
        .tls(tls)
        .build();

    Ok(smtp_transport)
}

#[tokio::main]
async fn main() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::SystemTime)
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Starting email server...");

    // 加载配置
    info!("Loading configuration from ./app_config.json");
    let app_config = get_app_config();
    info!("Configuration loaded successfully");

    // 创建 SMTP 传输
    info!(
        "Configuring SMTP transport for server: {}:{} with TLS",
        app_config.email.smtp_server, app_config.email.smtp_port
    );
    let smtp_transport = create_smtp_transport(&app_config.email).unwrap();
    info!("SMTP transport configured successfully");

    // 启动服务器
    let addr = format!(
        "{}:{}",
        app_config.server.server_host, app_config.server.server_port
    );
    info!("Server starting on {}", addr);

    // 创建应用状态
    let state = Arc::new(AppState {
        rate_limit: Mutex::new(RateLimit::new()),
        smtp_transport,
        app_config,
    });

    // 构建路由
    let app = Router::new()
        .route("/send-email", post(send_email))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("🎉 Server started successfully!");

    axum::serve(listener, app)
        // .serve(app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .await
        .unwrap();
}
