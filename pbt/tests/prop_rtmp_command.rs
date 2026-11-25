//! RTMP Command の Property-Based Testing

use noprop::TestCaseContext;
use shiguredo_rtmp::amf::AmfValue;
use shiguredo_rtmp::amf0::Amf0Value;
use shiguredo_rtmp::rtmp_command::{
    RtmpCommand, RtmpConnectCommand, RtmpCreateStreamCommand, RtmpDeleteStreamCommand,
    RtmpGetStreamLengthCommand, RtmpPlayCommand, RtmpPublishCommand, RtmpResultCommand,
    TransactionId,
};
use shiguredo_rtmp::rtmp_message::{RtmpMessage, RtmpMessageStreamId};

/// このファイルの PBT ケース数
const CASES: usize = 300;

/// 乱数シードを取得する環境変数名
const SEED_ENV: &str = "RTMP_RS_PBT_SEED";

/// 小さめの ASCII 文字列を生成する
///
/// 生成する文字は `[a-zA-Z0-9_./-]` に限定する。
fn sample_small_string(ctx: &mut TestCaseContext) -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_./-";
    let len = noprop::sample_usize_in(ctx, 0..=20);
    let mut s = String::new();
    for _ in 0..len {
        s.push(noprop::sample_choice(ctx, CHARS) as char);
    }
    s
}

/// TransactionId を生成する
fn sample_transaction_id(ctx: &mut TestCaseContext) -> TransactionId {
    let value = noprop::sample_u64_in(ctx, 0..=1000);
    TransactionId::from_f64(value as f64)
}

/// RtmpMessageStreamId を生成する
fn sample_stream_id(ctx: &mut TestCaseContext) -> RtmpMessageStreamId {
    RtmpMessageStreamId::new(noprop::sample_u32(ctx))
}

/// AMF 値を生成する
fn sample_amf_value(ctx: &mut TestCaseContext) -> AmfValue {
    match noprop::sample_weighted_index(ctx, &[1, 1, 1]) {
        0 => AmfValue::Amf0(Amf0Value::Null),
        1 => AmfValue::Amf0(Amf0Value::String(sample_small_string(ctx))),
        _ => AmfValue::Amf0(Amf0Value::Number(noprop::sample_f64(ctx))),
    }
}

/// connect の roundtrip を検証する
#[test]
fn connect_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let command = RtmpCommand::Connect(RtmpConnectCommand {
            app: app.clone(),
            flash_ver: flash_ver.clone(),
            tc_url: tc_url.clone(),
        });

        let message = command
            .into_pcm_message()
            .expect("connect コマンドのエンコードは失敗しない");
        let RtmpMessage::Command {
            name,
            transaction_id,
            object,
            args,
            ..
        } = message
        else {
            panic!("コマンドメッセージが期待される");
        };

        assert_eq!(name.as_str(), "connect", "コマンド名が一致しない");
        assert_eq!(
            transaction_id,
            TransactionId::CONNECT,
            "トランザクション ID が一致しない"
        );
        assert!(args.is_empty(), "connect の引数が空ではない");

        let decoded = RtmpCommand::from_message(&name, transaction_id, object, args)
            .expect("connect コマンドのデコードは失敗しない");
        match decoded {
            RtmpCommand::Connect(cmd) => {
                assert_eq!(cmd.app, app, "app が一致しない");
                assert_eq!(cmd.flash_ver, flash_ver, "flashVer が一致しない");
                assert_eq!(cmd.tc_url, tc_url, "tcUrl が一致しない");
            }
            _ => panic!("connect コマンドが期待される"),
        }
        Ok(())
    })?;
    Ok(())
}

/// createStream の roundtrip を検証する
#[test]
fn create_stream_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let transaction_id = sample_transaction_id(ctx);
        let command = RtmpCommand::CreateStream(RtmpCreateStreamCommand { transaction_id });

        let message = command
            .into_pcm_message()
            .expect("createStream コマンドのエンコードは失敗しない");
        let RtmpMessage::Command {
            name,
            transaction_id,
            object,
            args,
            ..
        } = message
        else {
            panic!("コマンドメッセージが期待される");
        };

        assert_eq!(name.as_str(), "createStream", "コマンド名が一致しない");
        assert!(args.is_empty(), "createStream の引数が空ではない");

        let decoded = RtmpCommand::from_message(&name, transaction_id, object, args)
            .expect("createStream コマンドのデコードは失敗しない");
        match decoded {
            RtmpCommand::CreateStream(cmd) => {
                assert_eq!(
                    cmd.transaction_id, transaction_id,
                    "トランザクション ID が一致しない"
                )
            }
            _ => panic!("createStream コマンドが期待される"),
        }
        Ok(())
    })?;
    Ok(())
}

/// publish の roundtrip を検証する
#[test]
fn publish_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);
        let command = RtmpCommand::Publish(RtmpPublishCommand {
            transaction_id,
            stream_name: stream_name.clone(),
        });

        let message = command
            .into_pcm_message()
            .expect("publish コマンドのエンコードは失敗しない");
        let RtmpMessage::Command {
            name,
            transaction_id,
            object,
            args,
            ..
        } = message
        else {
            panic!("コマンドメッセージが期待される");
        };

        assert_eq!(name.as_str(), "publish", "コマンド名が一致しない");

        let decoded = RtmpCommand::from_message(&name, transaction_id, object, args)
            .expect("publish コマンドのデコードは失敗しない");
        match decoded {
            RtmpCommand::Publish(cmd) => {
                assert_eq!(
                    cmd.transaction_id, transaction_id,
                    "トランザクション ID が一致しない"
                );
                assert_eq!(cmd.stream_name, stream_name, "ストリーム名が一致しない");
            }
            _ => panic!("publish コマンドが期待される"),
        }
        Ok(())
    })?;
    Ok(())
}

/// play の roundtrip を検証する
#[test]
fn play_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);
        let start = noprop::sample_f64(ctx);
        let command = RtmpCommand::Play(RtmpPlayCommand {
            transaction_id,
            stream_name: stream_name.clone(),
            start,
        });

        let message = command
            .into_pcm_message()
            .expect("play コマンドのエンコードは失敗しない");
        let RtmpMessage::Command {
            name,
            transaction_id,
            object,
            args,
            ..
        } = message
        else {
            panic!("コマンドメッセージが期待される");
        };

        assert_eq!(name.as_str(), "play", "コマンド名が一致しない");

        let decoded = RtmpCommand::from_message(&name, transaction_id, object, args)
            .expect("play コマンドのデコードは失敗しない");
        match decoded {
            RtmpCommand::Play(cmd) => {
                assert_eq!(
                    cmd.transaction_id, transaction_id,
                    "トランザクション ID が一致しない"
                );
                assert_eq!(cmd.stream_name, stream_name, "ストリーム名が一致しない");
                assert_eq!(cmd.start, start, "開始位置が一致しない");
            }
            _ => panic!("play コマンドが期待される"),
        }
        Ok(())
    })?;
    Ok(())
}

/// deleteStream の roundtrip を検証する
#[test]
fn delete_stream_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let transaction_id = sample_transaction_id(ctx);
        let stream_id = sample_stream_id(ctx);
        let command = RtmpCommand::DeleteStream(RtmpDeleteStreamCommand {
            transaction_id,
            stream_id,
        });

        let message = command
            .into_pcm_message()
            .expect("deleteStream コマンドのエンコードは失敗しない");
        let RtmpMessage::Command {
            name,
            transaction_id,
            object,
            args,
            ..
        } = message
        else {
            panic!("コマンドメッセージが期待される");
        };

        assert_eq!(name.as_str(), "deleteStream", "コマンド名が一致しない");

        let decoded = RtmpCommand::from_message(&name, transaction_id, object, args)
            .expect("deleteStream コマンドのデコードは失敗しない");
        match decoded {
            RtmpCommand::DeleteStream(cmd) => {
                assert_eq!(
                    cmd.transaction_id, transaction_id,
                    "トランザクション ID が一致しない"
                );
                assert_eq!(cmd.stream_id, stream_id, "ストリーム ID が一致しない");
            }
            _ => panic!("deleteStream コマンドが期待される"),
        }
        Ok(())
    })?;
    Ok(())
}

/// getStreamLength の roundtrip を検証する
#[test]
fn get_stream_length_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);
        let command = RtmpCommand::GetStreamLength(RtmpGetStreamLengthCommand {
            transaction_id,
            stream_name: stream_name.clone(),
        });

        let message = command
            .into_pcm_message()
            .expect("getStreamLength コマンドのエンコードは失敗しない");
        let RtmpMessage::Command {
            name,
            transaction_id,
            object,
            args,
            ..
        } = message
        else {
            panic!("コマンドメッセージが期待される");
        };

        assert_eq!(name.as_str(), "getStreamLength", "コマンド名が一致しない");

        let decoded = RtmpCommand::from_message(&name, transaction_id, object, args)
            .expect("getStreamLength コマンドのデコードは失敗しない");
        match decoded {
            RtmpCommand::GetStreamLength(cmd) => {
                assert_eq!(
                    cmd.transaction_id, transaction_id,
                    "トランザクション ID が一致しない"
                );
                assert_eq!(cmd.stream_name, stream_name, "ストリーム名が一致しない");
            }
            _ => panic!("getStreamLength コマンドが期待される"),
        }
        Ok(())
    })?;
    Ok(())
}

/// _result の roundtrip を検証する
#[test]
fn result_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let transaction_id = sample_transaction_id(ctx);
        let properties = sample_amf_value(ctx);
        let information = sample_amf_value(ctx);
        let command = RtmpCommand::Result(RtmpResultCommand {
            transaction_id,
            properties: properties.clone(),
            information: information.clone(),
        });

        let message = command
            .into_pcm_message()
            .expect("_result コマンドのエンコードは失敗しない");
        let RtmpMessage::Command {
            name,
            transaction_id,
            object,
            args,
            ..
        } = message
        else {
            panic!("コマンドメッセージが期待される");
        };

        assert_eq!(name.as_str(), "_result", "コマンド名が一致しない");

        let decoded = RtmpCommand::from_message(&name, transaction_id, object, args)
            .expect("_result コマンドのデコードは失敗しない");
        match decoded {
            RtmpCommand::Result(cmd) => {
                assert_eq!(
                    cmd.transaction_id, transaction_id,
                    "トランザクション ID が一致しない"
                );
                assert_eq!(cmd.properties, properties, "properties が一致しない");
                assert_eq!(cmd.information, information, "information が一致しない");
            }
            _ => panic!("_result コマンドが期待される"),
        }
        Ok(())
    })?;
    Ok(())
}

/// connect のトランザクション ID が不正な場合に拒否されることを検証する
#[test]
fn connect_invalid_transaction_id_rejected() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let transaction_id = noprop::sample_with_rejection(ctx, 8, |ctx| {
            let value = noprop::sample_u64_in(ctx, 0..=10);
            (value != 1).then_some(value)
        });
        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);

        let object = AmfValue::amf0_object([
            ("app", Amf0Value::String(app)),
            ("flashVer", Amf0Value::String(flash_ver)),
            ("tcUrl", Amf0Value::String(tc_url)),
        ]);
        let result = RtmpCommand::from_message(
            "connect",
            TransactionId::from_f64(transaction_id as f64),
            object,
            vec![],
        );
        assert!(
            result.is_err(),
            "不正なトランザクション ID の connect が受理された"
        );
        Ok(())
    })?;
    Ok(())
}

/// publish の引数が不足している場合に拒否されることを検証する
#[test]
fn publish_missing_args_rejected() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let transaction_id = sample_transaction_id(ctx);
        let result = RtmpCommand::from_message(
            "publish",
            transaction_id,
            AmfValue::Amf0(Amf0Value::Null),
            vec![],
        );
        assert!(result.is_err(), "引数が不足した publish が受理された");
        Ok(())
    })?;
    Ok(())
}

/// publish の種別が不正な場合に拒否されることを検証する
#[test]
fn publish_invalid_type_rejected() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);
        let publish_type = noprop::sample_with_rejection(ctx, 8, |ctx| {
            let value = sample_small_string(ctx);
            (value != "live").then_some(value)
        });
        let result = RtmpCommand::from_message(
            "publish",
            transaction_id,
            AmfValue::Amf0(Amf0Value::Null),
            vec![
                AmfValue::Amf0(Amf0Value::String(stream_name)),
                AmfValue::Amf0(Amf0Value::String(publish_type)),
            ],
        );
        assert!(result.is_err(), "不正な種別の publish が受理された");
        Ok(())
    })?;
    Ok(())
}

/// play で開始位置を省略すると仕様の既定値を使う
#[test]
fn play_missing_start_uses_default() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);
        let command = RtmpCommand::from_message(
            "play",
            transaction_id,
            AmfValue::Amf0(Amf0Value::Null),
            vec![AmfValue::Amf0(Amf0Value::String(stream_name.clone()))],
        )
        .expect("開始位置を省略した play は受理されるはず");
        let RtmpCommand::Play(command) = command else {
            panic!("play コマンドが返るはず");
        };
        // Adobe RTMP 仕様（2012 年 12 月版）7.2.2.1 節の既定値。仕様が変わる場合は見直す。
        assert_eq!(command.start, -2.0, "省略時は既定の開始位置を使うはず");
        assert_eq!(
            command.stream_name, stream_name,
            "ストリーム名を保持するはず"
        );
        assert_eq!(
            command.transaction_id, transaction_id,
            "トランザクション ID を保持するはず"
        );
        Ok(())
    })?;
    Ok(())
}

/// play の開始位置の型が不正な場合に拒否されることを検証する
#[test]
fn play_invalid_start_type_rejected() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);
        let result = RtmpCommand::from_message(
            "play",
            transaction_id,
            AmfValue::Amf0(Amf0Value::Null),
            vec![
                AmfValue::Amf0(Amf0Value::String(stream_name)),
                AmfValue::Amf0(Amf0Value::Null),
            ],
        );
        assert!(result.is_err(), "開始位置の型が不正な play が受理された");
        Ok(())
    })?;
    Ok(())
}

/// deleteStream のストリーム ID が不足している場合に拒否されることを検証する
#[test]
fn delete_stream_missing_id_rejected() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let transaction_id = sample_transaction_id(ctx);
        let result = RtmpCommand::from_message(
            "deleteStream",
            transaction_id,
            AmfValue::Amf0(Amf0Value::Null),
            vec![],
        );
        assert!(
            result.is_err(),
            "ストリーム ID が不足した deleteStream が受理された"
        );
        Ok(())
    })?;
    Ok(())
}
