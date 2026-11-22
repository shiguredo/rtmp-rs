//! RTMP URL のパス・クエリ・フラグメント境界の性質テスト

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
