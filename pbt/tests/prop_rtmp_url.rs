//! RTMP URL の authority・パス・クエリ・フラグメント境界の性質テスト

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

    #[test]
    fn bracketed_host_spelling_and_ports_are_preserved() -> noprop::TestResult {
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
            let host = format!("[2001:Db8:aB::{:X}]", noprop::sample_u16(ctx));
            for (scheme, default_port) in [("rtmp", 1935), ("rtmps", 443)] {
                // ポート省略時もブラケット・大文字小文字を保持し、整形から値を復元できる。
                for (api, result) in parse_authority(scheme, &host) {
                    let parsed = result.expect("ブラケットで囲んだホストは有効なはず");
                    assert_eq!(parsed.host, host, "{api} のホスト");
                    assert_eq!(parsed.port, default_port, "{api} の既定ポート");
                    assert_eq!(
                        RtmpUrl::parse(&parsed.to_string()).expect("整形した URL は有効なはず"),
                        parsed,
                        "ブラケット付きホストは整形と再解析で変化しない"
                    );
                }
            }
            Ok(())
        })?;
        Ok(())
    }
}
