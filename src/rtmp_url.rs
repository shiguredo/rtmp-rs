//! RTMP の URL を解析・整形するモジュール

use alloc::borrow::ToOwned;
use alloc::string::String;
use core::fmt;
use core::str::FromStr;

use crate::Error;

/// RTMP 用の URL
///
/// # 補足
///
/// [`core::str::FromStr`] の実装では [`RtmpUrl::parse()`] が使用されます。
/// もしストリーム名を URL 文字列とは別に指定したい場合には [`RtmpUrl::parse_with_stream_name()`] を使用してください。
///
/// [`fmt::Display`] はポートを常に明示し、保持している名前を再エスケープせずに出力します。
///
/// # ホストとポートの解析
///
/// IPv6 のように `:` を含むホストは、ホスト全体を一対の `[` と `]` で囲む必要があります。
/// ホスト名およびブラケットの内側は非空でなければなりませんが、その書式までは検証しません。
/// ユーザー情報はサポートしないため、authority にエスケープされていない `@` がある URL は拒否します。
/// このとき、入力値は解析エラーに含めません。
///
/// ポートを明示する場合は、ホストの後ろに `:` と `u16` に変換できる値が必要です。
/// ブラケットのないホストに続くポート番号は ASCII 数字に限ります。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RtmpUrl {
    /// RTMP サーバーのホスト名または IP アドレス。URL から解析したブラケット付きホストは元の表記を保持します。
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

impl RtmpUrl {
    /// ストリーム名を含む RTMP URL をパースします: `rtmp[s]://host[:port]/app/stream_name`
    ///
    /// パス部分に複数の `/` が含まれる場合、最後の `/` でアプリケーション名とストリーム名に分割されます。
    /// パス部分に非空のアプリケーション名とストリーム名が必要です。
    /// クエリは区切りの `?` ごとストリーム名に残し、フラグメントは破棄します。
    /// パーセントデコードは行いません。
    ///
    /// ポートが省略された場合、デフォルトポートが使用されます:
    /// - rtmp: 1935
    /// - rtmps: 443
    ///
    /// # エラー
    ///
    /// URL の形式が無効な場合や、パス部分に必要な名前がない場合は
    /// [`crate::error::ErrorKind::InvalidInput`] を返します。
    pub fn parse(s: &str) -> Result<Self, Error> {
        let (url_without_suffix, query_suffix) = split_query_and_discard_fragment(s);
        let (tls, host, port, path) = Self::parse_scheme_and_host_port(url_without_suffix)?;

        let (app, stream_name) = path
            .rsplit_once('/')
            .ok_or_else(|| Error::invalid_input("missing app and/or stream_name in path"))?;
        if app.is_empty() {
            return Err(Error::invalid_input("app name cannot be empty"));
        }
        if stream_name.is_empty() {
            return Err(Error::invalid_input("stream name cannot be empty"));
        }

        Ok(RtmpUrl {
            host: host.to_owned(),
            port,
            app: app.to_owned(),
            stream_name: [stream_name, query_suffix].concat(),
            tls,
        })
    }

    /// ストリーム名を別途指定して RTMP URL をパースします: `rtmp[s]://host[:port]/app`
    ///
    /// 最初の `/` を除いた非空のパス全体をアプリケーション名とし、クエリは区切りの `?` ごと残します。
    /// フラグメントは破棄し、パーセントデコードは行いません。別途指定された非空のストリーム名は加工しません。
    /// アプリケーション名にクエリが残る場合や、別途指定した名前に `/`、`?`、`#` が含まれる場合、
    /// [`fmt::Display`] と [`Self::parse()`] の往復は保証しません。
    ///
    /// ポートが省略された場合、デフォルトポートが使用されます:
    /// - rtmp: 1935
    /// - rtmps: 443
    ///
    /// # エラー
    ///
    /// URL の形式が無効な場合や、パス部分または別途指定したストリーム名が空の場合は
    /// [`crate::error::ErrorKind::InvalidInput`] を返します。
    pub fn parse_with_stream_name(s: &str, stream_name: &str) -> Result<Self, Error> {
        let (url_without_suffix, query_suffix) = split_query_and_discard_fragment(s);
        let (tls, host, port, app) = Self::parse_scheme_and_host_port(url_without_suffix)?;

        if app.is_empty() {
            return Err(Error::invalid_input("app name cannot be empty"));
        }
        if stream_name.is_empty() {
            return Err(Error::invalid_input("stream name cannot be empty"));
        }

        Ok(RtmpUrl {
            host: host.to_owned(),
            port,
            app: [app, query_suffix].concat(),
            stream_name: stream_name.to_owned(),
            tls,
        })
    }

    fn parse_scheme_and_host_port(s: &str) -> Result<(bool, &str, u16, &str), Error> {
        // スキーム部分
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

        // 呼び出し元でクエリとフラグメントを外しているため、最初の `/` で分けられる。
        let (authority, path) = rest
            .split_once('/')
            .ok_or_else(|| Error::invalid_input("missing '/' separator for app name"))?;

        let (host, port) = parse_host_port(authority, tls)?;

        if host.is_empty() {
            return Err(Error::invalid_input("host cannot be empty"));
        }

        Ok((tls, host, port, path))
    }
}

impl fmt::Display for RtmpUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scheme = if self.tls { "rtmps" } else { "rtmp" };
        write!(
            f,
            "{}://{}:{}/{}/{}",
            scheme, self.host, self.port, self.app, self.stream_name
        )
    }
}

impl FromStr for RtmpUrl {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

fn split_query_and_discard_fragment(s: &str) -> (&str, &str) {
    // RFC 3986 の §3.2 - §3.5 では、クエリとフラグメントの `/` はパスの区切りにならない。
    // フラグメント内の `?` を誤認しないよう先にフラグメントを除く。後継仕様を採用するときはこの分離順を見直す。
    let without_fragment = s.split_once('#').map_or(s, |(before, _)| before);
    without_fragment
        .find('?')
        .map_or((without_fragment, ""), |index| {
            without_fragment.split_at(index)
        })
}

fn parse_host_port(authority: &str, tls: bool) -> Result<(&str, u16), Error> {
    // RFC 3986 の §3.2.1 - §3.2.3 を境界の根拠とする。後継仕様を採用するときは見直す。
    // `@` はユーザー情報の区切り。未対応の値をエラーに含めない。
    if authority.contains('@') {
        return Err(Error::invalid_input(
            "userinfo is not supported in RTMP URLs",
        ));
    }

    // ホストの中身は検証せず、ブラケットの構造とポートの境界だけを確認する。
    let (host, port_str) = if authority.starts_with('[') {
        let bracket_end = authority
            .find(']')
            .ok_or_else(|| Error::invalid_input("invalid bracketed host, missing ']'"))?;
        let address = &authority[1..bracket_end];
        if address.is_empty() || address.contains('[') {
            return Err(Error::invalid_input("invalid bracketed host"));
        }

        let (host, remainder) = authority.split_at(bracket_end + 1);
        // ブラケット付きの明示ポートは従来どおり字句を絞らず、`u16` の変換に委ねる。
        let port_str = if remainder.is_empty() {
            None
        } else {
            Some(remainder.strip_prefix(':').ok_or_else(|| {
                Error::invalid_input(
                    "invalid format after bracketed host, expected ':' before port",
                )
            })?)
        };
        (host, port_str)
    } else {
        if authority.contains(['[', ']']) {
            return Err(Error::invalid_input("unexpected bracket in authority"));
        }
        if let Some((host, port_str)) = authority.split_once(':') {
            // 最初の `:` だけを区切りにすれば、複数の `:` は数字の検査で拒否される。
            if !port_str.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(Error::invalid_input("invalid host:port format"));
            }
            (host, Some(port_str))
        } else {
            (authority, None)
        }
    };

    // ポート番号をパース
    let port = match port_str {
        Some(port_s) => port_s
            .parse::<u16>()
            .map_err(|e| Error::invalid_input(format!("invalid port number '{port_s}': {e}")))?,
        None => {
            if tls {
                443
            } else {
                1935
            }
        }
    };

    Ok((host, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    use alloc::string::ToString;
    use core::str::FromStr;

    #[test]
    fn test_basic_rtmp_url() {
        let url =
            RtmpUrl::from_str("rtmp://example.com:1935/live/stream").expect("有効な RTMP URL");
        assert_eq!(url.host, "example.com");
        assert_eq!(url.port, 1935);
        assert_eq!(url.app, "live");
        assert_eq!(url.stream_name, "stream");
        assert!(!url.tls);
    }

    #[test]
    fn test_rtmps_url() {
        let url =
            RtmpUrl::from_str("rtmps://example.com:443/live/stream").expect("有効な RTMP URL");
        assert_eq!(url.host, "example.com");
        assert_eq!(url.port, 443);
        assert_eq!(url.app, "live");
        assert_eq!(url.stream_name, "stream");
        assert!(url.tls);
    }

    #[test]
    fn test_default_port_rtmp() {
        let url = RtmpUrl::from_str("rtmp://example.com/live/stream").expect("有効な RTMP URL");
        assert_eq!(url.port, 1935);
    }

    #[test]
    fn test_default_port_rtmps() {
        let url = RtmpUrl::from_str("rtmps://example.com/live/stream").expect("有効な RTMP URL");
        assert_eq!(url.port, 443);
    }

    #[test]
    fn test_nested_app_path() {
        let url = RtmpUrl::from_str("rtmp://example.com/app/path/stream").expect("有効な RTMP URL");
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
        let url = RtmpUrl::from_str(original).expect("有効な RTMP URL");
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
        let url =
            RtmpUrl::from_str("rtmp://192.168.1.1:1935/live/stream").expect("有効な RTMP URL");
        assert_eq!(url.host, "192.168.1.1");
    }

    #[test]
    fn test_clone_and_equality() {
        let url1 = RtmpUrl::from_str("rtmp://example.com/live/stream").expect("有効な RTMP URL");
        let url2 = url1.clone();
        assert_eq!(url1, url2);
    }

    // IPv6 対応テストケース
    #[test]
    fn test_ipv6_address_with_port() {
        let url = RtmpUrl::from_str("rtmp://[::1]:1935/live/stream").expect("有効な RTMP URL");
        assert_eq!(url.host, "[::1]");
        assert_eq!(url.port, 1935);
        assert_eq!(url.app, "live");
        assert_eq!(url.stream_name, "stream");
    }

    #[test]
    fn test_ipv6_address_full() {
        let url =
            RtmpUrl::from_str("rtmp://[2001:db8::1]:1935/live/stream").expect("有効な RTMP URL");
        assert_eq!(url.host, "[2001:db8::1]");
        assert_eq!(url.port, 1935);
    }

    #[test]
    fn test_ipv6_address_default_port_rtmp() {
        let url = RtmpUrl::from_str("rtmp://[::1]/live/stream").expect("有効な RTMP URL");
        assert_eq!(url.host, "[::1]");
        assert_eq!(url.port, 1935);
    }

    #[test]
    fn test_ipv6_address_default_port_rtmps() {
        let url = RtmpUrl::from_str("rtmps://[::1]/live/stream").expect("有効な RTMP URL");
        assert_eq!(url.host, "[::1]");
        assert_eq!(url.port, 443);
    }

    #[test]
    fn test_ipv6_display() {
        let url = RtmpUrl {
            host: "[::1]".to_owned(),
            port: 1935,
            app: "live".to_owned(),
            stream_name: "stream".to_owned(),
            tls: false,
        };
        assert_eq!(url.to_string(), "rtmp://[::1]:1935/live/stream");
    }

    #[test]
    fn test_ipv6_display_rtmps() {
        let url = RtmpUrl {
            host: "[2001:db8::1]".to_owned(),
            port: 443,
            app: "app".to_owned(),
            stream_name: "stream".to_owned(),
            tls: true,
        };
        assert_eq!(url.to_string(), "rtmps://[2001:db8::1]:443/app/stream");
    }

    #[test]
    fn test_ipv6_round_trip() {
        let original = "rtmp://[::1]:1935/live/stream";
        let url = RtmpUrl::from_str(original).expect("有効な RTMP URL");
        assert_eq!(url.to_string(), original);
    }

    #[test]
    fn test_invalid_ipv6_address() {
        // IPv6 部分の検証は RtmpUrl は行わないので成功する
        let result = RtmpUrl::from_str("rtmp://[::gggg]:1935/live/stream");
        assert!(result.is_ok());
    }

    #[test]
    fn test_ipv6_missing_closing_bracket() {
        let result = RtmpUrl::from_str("rtmp://[::1:1935/live/stream");
        assert!(result.is_err());
    }
}
