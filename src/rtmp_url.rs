use crate::Error;

/// RTMP 用の URL
///
/// この構造体は `rtmp://host:port/app/stream_name` または `rtmps://host:port/app/stream_name` 形式の URL に対応しています:
/// - ポート番号が省略された場合、rtmp は 1935、rtmps は 443 がデフォルトで使用されます
/// - パス部分に複数の `/` が含まれる場合、最後の `/` で app と stream_name に分割されます
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RtmpUrl {
    /// RTMP サーバーのホスト名または IP アドレス
    pub host: String,

    /// RTMP サーバーのポート番号
    pub port: u16,

    /// RTMP アプリケーション名
    pub app: String,

    /// ストリーム名
    pub stream_name: String,

    /// TLS 接続を使用するかどうか (rtmps の場合 true)
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

        // host[:port]/app/stream_name
        let (host_port, path) = rest
            .split_once('/')
            .ok_or_else(|| Error::invalid_input("missing '/' separator for app name"))?;

        // host, port
        let (host, port) = if let Some((host_part, port_part)) = host_port.rsplit_once(':') {
            let port = port_part.parse::<u16>().map_err(|e| {
                Error::invalid_input(format!("invalid port number '{port_part}': {e}"))
            })?;
            (host_part, port)
        } else {
            // ポート番号が未指定の場合はデフォルト値を使う
            (host_port, if tls { 443 } else { 1935 })
        };
        if host.is_empty() {
            return Err(Error::invalid_input("host cannot be empty"));
        }

        // app, stream_name
        let (app, stream_name) = path
            .rsplit_once('/')
            .ok_or_else(|| Error::invalid_input("Missing app and/or stream_name in path"))?;
        if app.is_empty() {
            return Err(Error::invalid_input("App name cannot be empty"));
        }
        if stream_name.is_empty() {
            return Err(Error::invalid_input("Stream name cannot be empty"));
        }

        Ok(RtmpUrl {
            host: host.to_owned(),
            port,
            app: app.to_owned(),
            stream_name: stream_name.to_owned(),
            tls,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::str::FromStr;

    #[test]
    fn test_basic_rtmp_url() {
        let url = RtmpUrl::from_str("rtmp://example.com:1935/live/stream").unwrap();
        assert_eq!(url.host, "example.com");
        assert_eq!(url.port, 1935);
        assert_eq!(url.app, "live");
        assert_eq!(url.stream_name, "stream");
        assert!(!url.tls);
    }

    #[test]
    fn test_rtmps_url() {
        let url = RtmpUrl::from_str("rtmps://example.com:443/live/stream").unwrap();
        assert_eq!(url.host, "example.com");
        assert_eq!(url.port, 443);
        assert_eq!(url.app, "live");
        assert_eq!(url.stream_name, "stream");
        assert!(url.tls);
    }

    #[test]
    fn test_default_port_rtmp() {
        let url = RtmpUrl::from_str("rtmp://example.com/live/stream").unwrap();
        assert_eq!(url.port, 1935);
    }

    #[test]
    fn test_default_port_rtmps() {
        let url = RtmpUrl::from_str("rtmps://example.com/live/stream").unwrap();
        assert_eq!(url.port, 443);
    }

    #[test]
    fn test_nested_app_path() {
        let url = RtmpUrl::from_str("rtmp://example.com/app/path/stream").unwrap();
        assert_eq!(url.app, "app/path");
        assert_eq!(url.stream_name, "stream");
    }

    #[test]
    fn test_display() {
        let url = RtmpUrl {
            host: "example.com".to_owned(),
            port: 1935,
            app: "live".to_owned(),
            stream_name: "stream".to_owned(),
            tls: false,
        };
        assert_eq!(url.to_string(), "rtmp://example.com:1935/live/stream");
    }

    #[test]
    fn test_display_rtmps() {
        let url = RtmpUrl {
            host: "example.com".to_owned(),
            port: 443,
            app: "live".to_owned(),
            stream_name: "stream".to_owned(),
            tls: true,
        };
        assert_eq!(url.to_string(), "rtmps://example.com:443/live/stream");
    }

    #[test]
    fn test_round_trip() {
        let original = "rtmp://example.com:1935/live/stream";
        let url = RtmpUrl::from_str(original).unwrap();
        assert_eq!(url.to_string(), original);
    }

    #[test]
    fn test_invalid_scheme() {
        let result = RtmpUrl::from_str("http://example.com/live/stream");
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_scheme_separator() {
        let result = RtmpUrl::from_str("rtmp example.com/live/stream");
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_path_separator() {
        let result = RtmpUrl::from_str("rtmp://example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_host() {
        let result = RtmpUrl::from_str("rtmp://:1935/live/stream");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_app() {
        let result = RtmpUrl::from_str("rtmp://example.com//stream");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_stream_name() {
        let result = RtmpUrl::from_str("rtmp://example.com/live/");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_port() {
        let result = RtmpUrl::from_str("rtmp://example.com:invalid/live/stream");
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_stream_name() {
        let result = RtmpUrl::from_str("rtmp://example.com/live");
        assert!(result.is_err());
    }

    #[test]
    fn test_ipv4_address() {
        let url = RtmpUrl::from_str("rtmp://192.168.1.1:1935/live/stream").unwrap();
        assert_eq!(url.host, "192.168.1.1");
    }

    #[test]
    fn test_clone_and_equality() {
        let url1 = RtmpUrl::from_str("rtmp://example.com/live/stream").unwrap();
        let url2 = url1.clone();
        assert_eq!(url1, url2);
    }
}
