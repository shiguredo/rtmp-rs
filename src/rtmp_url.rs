use crate::Error;

/// RTMP 用の URL
///
/// TODO: 対応している形式について簡単に書く
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RtmpUrl {
    // TODO: 各フィールドの説明を簡単に書く
    pub host: String,
    pub port: u16,
    pub app: String,
    pub stream_name: String,
    pub tls: bool,
}

impl std::fmt::Display for RtmpUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let scheme = if self.tls { "rtmps" } else { "rtmp" };
        write!(
            f,
            "{}://{}:{}/{}/{}",
            scheme, self.host, self.port, self.app, self.stream_name
        )
    }
}

impl std::str::FromStr for RtmpUrl {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // scheme://
        let (scheme, rest) = s
            .split_once("://")
            .ok_or_else(|| Error::invalid_input("missing '://' in RTMP URL"))?;
        let tls = match scheme {
            "rtmp" => false,
            "rtmps" => true,
            _ => {
                return Err(Error::invalid_input(format!(
                    "invalid scheme '{scheme}', expected 'rtmp' or 'rtmps'"
                )));
            }
        };

        // host:port/app/stream_name

        let (host_port, path) = rest
            .split_once('/')
            .ok_or_else(|| Error::invalid_input("missing '/' separator for app name"))?;
        let (host, port) = host_port
            .rsplit_once(':')
            .ok_or_else(|| Error::invalid_input("missing ':' separator for port"))?;
        if host.is_empty() {
            return Err(Error::invalid_input("host cannot be empty"));
        }

        let port = port
            .parse::<u16>()
            .map_err(|_| Error::invalid_input(format!("Invalid port number '{}'", port)))?;

        // Parse app and stream_name
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() < 2 {
            return Err(Error::invalid_input(
                "Missing app and/or stream_name in path",
            ));
        }

        let app = parts[0].to_string();
        let stream_name = parts[1..].join("/"); // Support nested paths

        if app.is_empty() {
            return Err(Error::invalid_input("App name cannot be empty"));
        }

        if stream_name.is_empty() {
            return Err(Error::invalid_input("Stream name cannot be empty"));
        }

        Ok(RtmpUrl {
            host: host.to_string(),
            port,
            app,
            stream_name,
            tls,
        })
    }
}
