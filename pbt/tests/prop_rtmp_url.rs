//! RTMP URL の往復と authority・パス・クエリ・フラグメント境界の性質テスト

use std::net::Ipv4Addr;

use noprop::TestCaseContext;
use shiguredo_rtmp::error::{Error, ErrorKind};
use shiguredo_rtmp::rtmp_url::RtmpUrl;

const CASES: usize = 200;
const SEED_ENV: &str = "RTMP_RS_PBT_SEED";
const BASE_URL: &str = "rtmp://example.com";

const PATH_TOKENS: &[&str] = &["a", "Z", "9", "-", "%2F", "%3F", "%23"];
const SUFFIX_TOKENS: &[&str] = &["a", "Z", "9", "=", "&", "/", "?", "%2F", "%3F", "%23"];

/// 指定された部分文字列の候補から、非空の文字列を生成する。
fn sample_text(ctx: &mut TestCaseContext, tokens: &[&'static str]) -> String {
    let len = noprop::sample_usize_in(ctx, 1..=12);
    (0..len)
        .map(|_| noprop::sample_choice(ctx, tokens))
        .collect()
}

fn assert_url_names(app: &str, stream: &str, query: &str, fragment: &str) {
    let input = format!("{BASE_URL}/{app}/{stream}{query}{fragment}");
    let parsed = RtmpUrl::parse(&input).expect("非空の app と stream があれば解析できるはず");
    assert_eq!(parsed.app, app, "クエリとフラグメントは app に含めない");
    assert_eq!(
        parsed.stream_name,
        format!("{stream}{query}"),
        "クエリだけを stream に残す"
    );
    assert_eq!(
        parsed.to_string(),
        format!("{BASE_URL}:1935/{app}/{stream}{query}"),
        "整形した URL にはクエリを残し、フラグメントを含めない"
    );

    // 別引数は URL の一部ではない。クエリは app に残し、渡した名前はそのまま保持する。
    let input = format!("{BASE_URL}/{app}{query}{fragment}");
    let separate_stream = format!("{stream}?x/y#z");
    let parsed = RtmpUrl::parse_with_stream_name(&input, &separate_stream)
        .expect("非空の app と別引数の名前があれば解析できるはず");
    assert_eq!(
        parsed.app,
        format!("{app}{query}"),
        "パス全体とクエリを app に残す"
    );
    assert_eq!(
        parsed.stream_name, separate_stream,
        "別引数の名前は加工しない"
    );
}

#[test]
fn query_and_fragment_preserve_path_boundaries() -> noprop::TestResult {
    // スラッシュ入りクエリの既知の例と、フラグメント付きの別引数の例を必ず通す。
    let example_query = "?token=ab/cd";
    assert_url_names("live", "stream", example_query, "");
    assert_url_names("live", "stream", example_query, "#section/a");
    let example = format!("{BASE_URL}/live/stream{example_query}");
    assert_eq!(
        example
            .parse::<RtmpUrl>()
            .expect("FromStr でも解析できるはず"),
        RtmpUrl::parse(&example).expect("解析できるはず"),
        "FromStr と parse は同じ結果を返す"
    );

    // スラッシュのないクエリの互換性と、パーセント表記および UTF-8 境界を固定する。
    assert_url_names("live", "stream", "?quality=high", "");
    assert_url_names("配信%2f", "映像", "?mode=%41実演", "#節");

    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let app = format!(
            "{}/{}",
            sample_text(ctx, PATH_TOKENS),
            sample_text(ctx, PATH_TOKENS)
        );
        let stream = sample_text(ctx, PATH_TOKENS);
        let query = format!("?ref={}/next?key=value", sample_text(ctx, SUFFIX_TOKENS));
        let trailing_slash_query = format!("?{}/", sample_text(ctx, PATH_TOKENS));
        let fragment = format!("#section/{}?part", sample_text(ctx, SUFFIX_TOKENS));

        // 等号のないクエリも保持する。区切りだけの場合も、それぞれ未指定とは区別する。
        for (query, fragment) in [
            (query.as_str(), ""),
            (trailing_slash_query.as_str(), ""),
            ("", fragment.as_str()),
            (query.as_str(), fragment.as_str()),
            ("?", ""),
            ("", "#"),
            ("?", "#"),
        ] {
            assert_url_names(&app, &stream, query, fragment);
        }
        Ok(())
    })?;
    Ok(())
}

fn assert_invalid_input(result: Result<RtmpUrl, Error>, input: &str, missing: &str) {
    assert!(
        matches!(result, Err(ref error) if error.kind == ErrorKind::InvalidInput),
        "{missing}。URL {input:?} は InvalidInput のはず: {result:?}"
    );
}

#[test]
fn query_and_fragment_cannot_supply_missing_names() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let name = sample_text(ctx, PATH_TOKENS);
        let suffix = sample_text(ctx, SUFFIX_TOKENS);
        // 末尾にパスに見える文字列があっても、欠けた app や stream を補ってはいけない。
        for suffix in [
            format!("?ref={suffix}/app/stream"),
            format!("#section/{suffix}?part/stream"),
            format!("?ref={suffix}/app#section/?part/stream"),
        ] {
            for path in [
                "",
                "/",
                &format!("/{name}"),
                &format!("//{name}"),
                &format!("/{name}/"),
            ] {
                let input = format!("{BASE_URL}{path}{suffix}");
                assert_invalid_input(
                    RtmpUrl::parse(&input),
                    &input,
                    "parse に必要なパス部分がない",
                );
            }
            for path in ["", "/"] {
                let input = format!("{BASE_URL}{path}{suffix}");
                assert_invalid_input(
                    RtmpUrl::parse_with_stream_name(&input, &name),
                    &input,
                    "parse_with_stream_name に必要な app がない",
                );
            }
            // パスが有効でも、ストリーム名を別引数で渡す場合は空文字列を拒否する。
            let input = format!("{BASE_URL}/{name}{suffix}");
            assert_invalid_input(
                RtmpUrl::parse_with_stream_name(&input, ""),
                &input,
                "parse_with_stream_name の別引数が空",
            );
        }
        Ok(())
    })?;
    Ok(())
}

/// クエリやフラグメントのない URL と、`/`、`?`、`#` を含まない別引数を使い、
/// 両解析 API と明示ポート付きの整形・再解析で全フィールドを保持することを確認する。
#[test]
fn roundtrip_preserves_all_url_fields() -> noprop::TestResult {
    // 別引数は既存のストリーム名の置き換えではなく、パス全体が app になる。
    let parsed =
        RtmpUrl::parse_with_stream_name("rtmp://example.invalid/live/original", "replacement")
            .expect("複数階層の app と別引数の名前を解析できるはず");
    assert_eq!(
        (parsed.app.as_str(), parsed.stream_name.as_str()),
        ("live/original", "replacement"),
        "URL の最後のセグメントも app に残し、別引数を stream に使うはず"
    );

    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ホストの 3 形式を毎回試す。IPv6 のブラケットと大文字小文字も元の表記を保持する。
        let hosts = [
            format!("host-{}.example.invalid", noprop::sample_u16(ctx)),
            Ipv4Addr::from(noprop::sample_u32(ctx)).to_string(),
            format!("[2001:Db8:aB::{:X}]", noprop::sample_u16(ctx)),
        ];
        let app = (0..noprop::sample_usize_in(ctx, 1..=4))
            .map(|_| sample_text(ctx, PATH_TOKENS))
            .collect::<Vec<_>>()
            .join("/");
        let stream = sample_text(ctx, PATH_TOKENS);
        let explicit = noprop::sample_u16(ctx);

        // スキームとポートの指定・省略を全てのホスト形式と組み合わせる。
        for host in &hosts {
            for (scheme, tls, default_port) in [("rtmp", false, 1935), ("rtmps", true, 443)] {
                for input_port in [None, Some(explicit)] {
                    let authority =
                        input_port.map_or_else(|| host.clone(), |p| format!("{host}:{p}"));
                    let app_url = format!("{scheme}://{authority}/{app}");
                    let stream_url = format!("{app_url}/{stream}");
                    // 解析結果から期待値を作らず、入力を組み立てた各要素と既定ポートを使う。
                    let port = input_port.unwrap_or(default_port);
                    let expected = RtmpUrl {
                        host: host.clone(),
                        port,
                        app: app.clone(),
                        stream_name: stream.clone(),
                        tls,
                    };
                    let parsed = RtmpUrl::parse(&stream_url).unwrap_or_else(|error| {
                        panic!("parse は {stream_url:?} を解析できるはず: {error}")
                    });
                    assert_eq!(parsed, expected, "parse で {stream_url:?} の全フィールドが一致するはず");
                    let separate = RtmpUrl::parse_with_stream_name(&app_url, &stream)
                        .unwrap_or_else(|error| {
                            panic!(
                                "parse_with_stream_name は {app_url:?} と {stream:?} を解析できるはず: {error}"
                            )
                        });
                    assert_eq!(
                        separate, expected,
                        "parse_with_stream_name で {app_url:?} と {stream:?} の全フィールドが一致するはず"
                    );

                    // 両 API の結果は同じ値なので、整形と再解析は組み合わせごとに 1 回だけ行う。
                    let display = parsed.to_string();
                    assert_eq!(
                        display,
                        format!("{scheme}://{host}:{port}/{app}/{stream}"),
                        "整形ではポートを明示するはず"
                    );
                    let reparsed = RtmpUrl::parse(&display).unwrap_or_else(|error| {
                        panic!("整形した URL {display:?} を再解析できるはず: {error}")
                    });
                    assert_eq!(reparsed, expected, "整形と再解析で全フィールドを保持するはず");
                }
            }
        }
        Ok(())
    })?;
    Ok(())
}

mod authority {
    use super::{CASES, Error, ErrorKind, RtmpUrl, SEED_ENV, TestCaseContext};

    fn parse_authority(
        scheme: &str,
        authority: &str,
    ) -> [(&'static str, Result<RtmpUrl, Error>); 2] {
        [
            (
                "parse",
                RtmpUrl::parse(&format!("{scheme}://{authority}/app/stream")),
            ),
            (
                "parse_with_stream_name",
                RtmpUrl::parse_with_stream_name(&format!("{scheme}://{authority}/app"), "stream"),
            ),
        ]
    }

    fn assert_malformed_authority(authority: &str) {
        for (api, result) in parse_authority("rtmp", authority) {
            assert!(
                matches!(&result, Err(error) if error.kind == ErrorKind::InvalidInput),
                "{api} は不正な authority {authority:?} を拒否するはず: {result:?}"
            );
        }
    }

    #[test]
    fn malformed_host_structures_are_rejected() -> noprop::TestResult {
        assert_malformed_authority("[]");

        let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
        noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
            // 末尾の IPv6 セグメントは 10 進数字だけで表記し、ポートと誤認しやすい形を生成する。
            let suffix = noprop::sample_u16_in(ctx, 0..=9999);
            let ip = format!("2001:db8::{suffix}");
            let host = format!("host{}.invalid", noprop::sample_u16(ctx));
            // 内側の `[`、余分な `]`、ポートの区切り欠落を別々に検証する。
            for authority in [
                format!("::{suffix}"),
                format!("2001:db8:0:0:0:0:0:{suffix}"),
                format!("[{ip}"),
                format!("[2001:[db8::{suffix}]"),
                format!("[{ip}]]"),
                format!("[{ip}]1935"),
                format!("{host}["),
                format!("{host}]:1935"),
                ip,
            ] {
                assert_malformed_authority(&authority);
            }
            Ok(())
        })?;
        Ok(())
    }

    #[test]
    fn unsupported_userinfo_is_rejected_without_echoing_values() -> noprop::TestResult {
        let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
        noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
            // 実在の認証情報ではなく合成した値で、拒否時に入力をエラーへ持ち込まないことを確認する。
            let id = noprop::sample_u16(ctx);
            let user = format!("synthetic-user-{id}");
            let dummy = format!("synthetic-password-{id}");
            let host = format!("host%40{id}.invalid");
            // 生の `@` はブラケット内や明示ポート部分でも拒否し、同じ authority の `%40` には影響されない。
            for (case, authority) in [
                ("ユーザー名のみ", format!("{user}@{host}")),
                ("パスワード付き", format!("{user}:{dummy}@{host}")),
                ("ブラケット内", format!("[2001:db8::{user}@{dummy}]")),
                ("明示ポート部分", format!("[::1]:{user}@{dummy}")),
            ] {
                for (api, result) in parse_authority("rtmp", &authority) {
                    let Err(error) = result else {
                        panic!("{api} は {case} の生の `@` を拒否するはず");
                    };
                    assert_eq!(
                        error.kind,
                        ErrorKind::InvalidInput,
                        "{api} の {case} のエラー種別"
                    );
                    assert!(
                        !error.reason.contains(&user) && !error.reason.contains(&dummy),
                        "{api} の {case} のエラーにユーザー情報の値を含めない"
                    );
                }
            }

            // エスケープされた `@` や authority の外側にある `@` は区切りにしない。
            let parsed = RtmpUrl::parse(&format!("rtmp://{host}/app@media/stream?tag=@sample"))
                .expect("authority 外の記号は許容されるはず");
            assert_eq!(parsed.host, host);
            assert_eq!(parsed.app, "app@media");
            assert_eq!(parsed.stream_name, "stream?tag=@sample");
            let parsed = RtmpUrl::parse_with_stream_name(
                &format!("rtmp://{host}/app@media?tag=@sample"),
                "stream@media",
            )
            .expect("クエリと別引数の記号は許容されるはず");
            assert_eq!(parsed.host, host);
            assert_eq!(parsed.app, "app@media?tag=@sample");
            assert_eq!(parsed.stream_name, "stream@media");
            Ok(())
        })?;
        Ok(())
    }

    /// 明示ポートを持つホストの表記、ポートの字句、`u16` の境界を検証する。
    #[test]
    fn explicit_ports_preserve_host_and_validate_range() -> noprop::TestResult {
        // 明示ポートを持つ基本形は、どちらの API でも同じホストとポートを返す。
        for (host, port_text, port) in [
            ("host", "1935", 1935),
            ("[::1]", "1935", 1935),
            // 明示ポートを無視する退行を検出するため、既定値でない値も使う。
            ("host", "1936", 1936),
            ("[::1]", "1936", 1936),
            // IPvFuture のように `:` を含まないリテラルも、構造だけを検証して表記を保持する。
            ("[v1.Example]", "1936", 1936),
            // `+` を許すのは、字句を直接 `u16` に変換するブラケット付きホストだけ。
            ("[::1]", "+1936", 1936),
        ] {
            for (api, result) in parse_authority("rtmp", &format!("{host}:{port_text}")) {
                let parsed = result.expect("明示ポートを持つ URL は有効なはず");
                assert_eq!(parsed.host, host, "{api} のホスト");
                assert_eq!(parsed.port, port, "{api} のポート");
            }
        }
        assert_malformed_authority("host:+1936");

        let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
        noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
            let host = format!("host-{}.example.invalid", noprop::sample_u16(ctx));
            let ipv6 = format!("[2001:db8::{:x}]", noprop::sample_u16(ctx));
            let invalid = Err(ErrorKind::InvalidInput);
            // ホストを生成し、ポートの境界値は毎回すべて照合する。
            // 0 は解析できるが、接続先ポートとして利用できるかどうかは試験しない。
            // 空の明示ポートは省略と異なり、既定ポートにはならない。
            let decimal_ports = [
                ("0", Ok(0)),
                ("65535", Ok(65535)),
                ("065535", Ok(65535)),
                ("65536", invalid),
                ("", invalid),
            ];
            // ブラケット付きでは事前の数字検査をせず、これらも整数変換で拒否する。
            let bracketed_only = [("-1", invalid), ("invalid", invalid)];
            for (host, extra) in [(&host, &[][..]), (&ipv6, bracketed_only.as_slice())] {
                for &(port_text, expected) in decimal_ports.iter().chain(extra) {
                    let authority = format!("{host}:{port_text}");
                    for scheme in ["rtmp", "rtmps"] {
                        for (api, result) in parse_authority(scheme, &authority) {
                            let actual = result.map(|url| url.port).map_err(|error| error.kind);
                            assert_eq!(
                                actual, expected,
                                "{api} の {scheme}://{authority} の明示ポート"
                            );
                        }
                    }
                }
            }
            Ok(())
        })?;
        Ok(())
    }
}
