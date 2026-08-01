use std::fs::File;
use std::io::{BufRead, BufReader};

pub const DIR_WORDLIST: &[&str] = &[
    "admin", "login", "login.php", "wp-admin", "wp-login.php", "wp-content", "wp-includes",
    "wp-json", "api", "api/v1", "api/v2", "v1", "v2", "dashboard", "panel", "cpanel", "phpmyadmin",
    "pma", "mysql", "db", "database", "config", "config.php", "configuration", "setup", "install",
    "install.php", "test", "tests", "testing", "dev", "development", "stage", "staging", "uat",
    "sandbox", "temp", "tmp", "backup", "bak", "old", "new", "backups", "uploads", "upload",
    "download", "downloads", "files", "assets", "static", "staticfiles", "css", "js", "images",
    "img", "media", "fonts", "icons", "favicon.ico", "robots.txt", "sitemap.xml", "sitemap_index.xml",
    "crossdomain.xml", "security.txt", ".well-known", ".well-known/security.txt", ".git", ".git/config",
    ".git/HEAD", ".env", ".env.local", ".env.production", ".htaccess", ".htpasswd", ".svn",
    ".DS_Store", "README", "README.md", "readme.html", "CHANGELOG", "changelog.txt", "LICENSE",
    "license.txt", "server-status", "server-info", "status", "info.php", "phpinfo.php", "test.php",
    "index.php", "index.html", "index.js", "main.js", "app.js", "bundle.js", "webpack.config.js",
    "package.json", "composer.json", "Gemfile", "Pipfile", "requirements.txt", "vendor",
    "node_modules", "src", "build", "dist", "public", "private", "docs", "doc", "help", "faq",
    "terms", "privacy", "about", "contact", "user", "users", "profile", "account", "accounts",
    "register", "signup", "signin", "logout", "reset", "forgot", "search", "cart", "checkout",
    "shop", "store", "products", "product", "category", "blog", "news", "article", "articles",
    "posts", "content", "pages", "page", "landing", "home", "index", "error", "404", "500",
    "error_log", "error.log", "debug.log", "access.log", "adminer.php", "db.php", "conn.php",
    "connection.php", "server.php", "xmlrpc.php", "web.config", "Dockerfile", "docker-compose.yml",
    ".dockerignore", ".github", ".circleci", ".travis.yml", "Jenkinsfile", "Makefile", "build.gradle",
    "pom.xml", "gradlew", "yarn.lock", "package-lock.json", "tsconfig.json", "graphql", "graphiql",
    "swagger", "swagger-ui", "api-docs", "redoc", "openapi.json", "actuator", "actuator/health",
    "actuator/env", "actuator/heapdump", "metrics", "health", "healthz", "readyz", "healthcheck",
    "version", "version.txt", "vite.config.js", "next.config.js", "nuxt.config.js", "svelte.config.js",
    "wp-config.php.bak", "config.php.bak", "index.php.bak", ".env.bak", "backup.zip", "backup.tar.gz",
    "db.sql", "dump.sql", "database.sql", "export.sql", "users.sql", "data.json", "users.json",
    "config.json", "settings.json", "secrets.json", "aws.json", "aws_credentials", "id_rsa",
    "id_rsa.pub", "authorized_keys", "ssh", "debug", "console", "shell", "cmd", "shell.php",
    "cmd.php", "c99.php", "r57.php", "webshell.php", "phpmyadmin2", "pydio", "webmail", "mail",
    "email", "ftp", "cgi-bin", "cgi-bin/test.cgi", "icons", "manual", "downloads", "cache",
    "cached", "storage", "data", "databases", "migrations", "seeders", "app", "application",
    "wp-content/uploads", "wp-content/plugins", "wp-content/themes", "xmlrpc", "webdav", "dav",
    "server-status/", "licenses", "keys", "ssl", "certs", "certificates", "logs", "log",
];

pub const SUBDOMAIN_WORDLIST: &[&str] = &[
    "www", "mail", "webmail", "smtp", "pop", "imap", "mx", "ns1", "ns2", "ns3", "dns", "dns1",
    "dns2", "ftp", "sftp", "ssh", "vpn", "remote", "rdp", "proxy", "gateway", "router", "nas",
    "files", "file", "upload", "download", "storage", "cloud", "drive", "drop", "share", "media",
    "cdn", "static", "assets", "img", "images", "photo", "photos", "video", "videos", "stream",
    "live", "tv", "radio", "music", "api", "api2", "api3", "api-internal", "dev-api", "staging-api",
    "gateway-api", "rest", "graphql", "ws", "wss", "socket", "websocket", "mqtt", "portal", "app",
    "apps", "application", "applications", "my", "me", "profile", "user", "users", "account",
    "accounts", "auth", "login", "signin", "signup", "register", "sso", "oauth", "id", "identity",
    "admin", "administrator", "panel", "dashboard", "manage", "management", "console", "control",
    "cpanel", "plesk", "webmin", "whm", "phpmyadmin", "mysql", "db", "database", "dbserver",
    "db01", "db02", "postgres", "postgresql", "mongodb", "mongo", "redis", "cache", "memcached",
    "elasticsearch", "es", "kibana", "grafana", "prometheus", "monitor", "monitoring", "metrics",
    "status", "uptime", "health", "healthcheck", "alerts", "logging", "logs", "log", "sentry",
    "jenkins", "ci", "cd", "build", "builder", "release", "test", "testing", "tests", "qa", "uat",
    "dev", "development", "develop", "stage", "staging", "preprod", "pre-prod", "demo", "sandbox",
    "beta", "alpha", "next", "new", "old", "legacy", "preview", "staging2", "dev2", "test2",
    "backup", "backups", "bak", "archive", "old2", "temp", "tmp", "temporary", "internal",
    "intranet", "corp", "corporate", "private", "secure", "security", "firewall", "fw", "edge",
    "lb", "load", "loadbalancer", "cluster", "node", "node1", "node2", "worker", "worker1",
    "web", "web01", "web02", "web1", "web2", "server", "server1", "srv", "srv1", "app1", "app2",
    "shop", "store", "ecommerce", "cart", "checkout", "payment", "pay", "billing", "invoice",
    "blog", "news", "forum", "community", "chat", "support", "help", "helpdesk", "ticket",
    "tickets", "wiki", "docs", "documentation", "doc", "kb", "knowledgebase", "learn", "training",
    "courses", "course", "e-learning", "lms", "student", "teacher", "school", "university",
    "events", "event", "calendar", "meetings", "meet", "zoom", "team", "teams", "work", "office",
    "hr", "employee", "staff", "people", "dir", "directory", "phone", "phones", "fax", "print",
    "printer", "scan", "scanner", "kiosk", "pos", "terminal", "vending", "iot", "device",
    "devices", "camera", "cameras", "cctv", "nvr", "dvr", "ipcam", "sensor", "sensors", "hub",
    "smart", "home", "automation", "voice", "phone", "voip", "sip", "asterisk", "pbx", "call",
    "calls", "contact", "contacts", "crm", "erp", "sap", "oracle", "odoo", "openerp", "pos",
    "inventory", "warehouse", "supply", "logistics", "ship", "shipping", "track", "tracking",
    "order", "orders", "invoice", "receipt", "finance", "accounting", "payroll", "tax", "legal",
    "compliance", "audit", "audits", "reports", "report", "analytics", "data", "datasets",
    "bigdata", "spark", "hadoop", "hive", "kafka", "rabbitmq", "queue", "worker", "cron",
    "scheduler", "batch", "jobs", "job", "queue1", "etl", "pipeline", "pipelines", "ml", "ai",
    "models", "model", "training", "inference", "gpu", "gpu01", "hpc", "compute", "cluster",
    "docker", "registry", "k8s", "kubernetes", "kube", "rancher", "openshift", "harbor",
    "gitlab", "gitlab-ci", "github", "bitbucket", "svn", "git", "repo", "repos", "repository",
    "source", "code", "codes", "codereview", "review", "stack", "overflow", "forums", "board",
    "boards", "poll", "survey", "surveys", "feedback", "form", "forms", "newsletter", "sub",
    "subscribe", "subscription", "api-gateway", "gateway-api", "kong", "traefik", "nginx",
    "apache", "www2", "www3", "m", "mobile", "mob", "touch", "tablet", "mweb", "wapp",
];

/// Load a wordlist from a file, falling back to the builtin list.
pub fn load_wordlist<'a>(path: &Option<String>, builtin: &'static [&'a str]) -> Vec<String> {
    match path {
        Some(p) if !p.is_empty() => {
            if let Ok(file) = File::open(p) {
                let reader = BufReader::new(file);
                let lines: Vec<String> = reader
                    .lines()
                    .filter_map(|l| l.ok())
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                    .collect();
                if !lines.is_empty() {
                    return lines;
                }
            }
            builtin.iter().map(|s| s.to_string()).collect()
        }
        _ => builtin.iter().map(|s| s.to_string()).collect(),
    }
}
