//! RTMP Command の Property-Based Testing

use proptest::prelude::*;
use shiguredo_rtmp::tests::{
    Amf0Value, AmfValue, RtmpCommand, RtmpMessage, RtmpMessageStreamId, TransactionId,
};

// =============================================================================
// Strategy 定義
// =============================================================================

/// 小さめの ASCII 文字列を生成する
fn arb_small_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_./-]{0,20}".prop_map(|s| s.to_string())
}

/// TransactionId を生成する
fn arb_transaction_id() -> impl Strategy<Value = TransactionId> {
    (0i64..=1000i64).prop_map(|v| TransactionId::from_f64(v as f64))
}

/// RtmpMessageStreamId を生成する
fn arb_stream_id() -> impl Strategy<Value = RtmpMessageStreamId> {
    any::<u32>().prop_map(RtmpMessageStreamId::new)
}

/// AMF 値を簡易的に生成する
fn arb_amf_value() -> impl Strategy<Value = AmfValue> {
    prop_oneof![
        Just(AmfValue::Amf0(Amf0Value::Null)),
        arb_small_string().prop_map(|s| AmfValue::Amf0(Amf0Value::String(s))),
        prop::num::f64::NORMAL.prop_map(|n| AmfValue::Amf0(Amf0Value::Number(n))),
    ]
}

// =============================================================================
// Roundtrip テスト
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// Connect の roundtrip を検証
    #[test]
    fn connect_roundtrip(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
    ) {
        let command = RtmpCommand::Connect(shiguredo_rtmp::tests::RtmpConnectCommand {
            app: app.clone(),
            flash_ver: flash_ver.clone(),
            tc_url: tc_url.clone(),
        });

        let message = command.into_pcm_message().expect("connect should encode");
        let RtmpMessage::Command { name, transaction_id, object, args, .. } = message else {
            return Err(TestCaseError::fail("expected command message"));
        };

        prop_assert_eq!(name.as_str(), "connect");
        prop_assert_eq!(transaction_id, TransactionId::CONNECT);
        prop_assert!(args.is_empty());

        let decoded = RtmpCommand::from_message(&name, transaction_id, object, args)
            .expect("connect should decode");
        match decoded {
            RtmpCommand::Connect(cmd) => {
                prop_assert_eq!(cmd.app, app);
                prop_assert_eq!(cmd.flash_ver, flash_ver);
                prop_assert_eq!(cmd.tc_url, tc_url);
            }
            _ => return Err(TestCaseError::fail("expected connect command")),
        }
    }

    /// CreateStream の roundtrip を検証
    #[test]
    fn create_stream_roundtrip(transaction_id in arb_transaction_id()) {
        let command = RtmpCommand::CreateStream(shiguredo_rtmp::tests::RtmpCreateStreamCommand {
            transaction_id,
        });

        let message = command.into_pcm_message().expect("createStream should encode");
        let RtmpMessage::Command { name, transaction_id, object, args, .. } = message else {
            return Err(TestCaseError::fail("expected command message"));
        };

        prop_assert_eq!(name.as_str(), "createStream");
        prop_assert!(args.is_empty());

        let decoded = RtmpCommand::from_message(&name, transaction_id, object, args)
            .expect("createStream should decode");
        match decoded {
            RtmpCommand::CreateStream(cmd) => {
                prop_assert_eq!(cmd.transaction_id, transaction_id);
            }
            _ => return Err(TestCaseError::fail("expected createStream command")),
        }
    }

    /// Publish の roundtrip を検証
    #[test]
    fn publish_roundtrip(
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
    ) {
        let command = RtmpCommand::Publish(shiguredo_rtmp::tests::RtmpPublishCommand {
            transaction_id,
            stream_name: stream_name.clone(),
        });

        let message = command.into_pcm_message().expect("publish should encode");
        let RtmpMessage::Command { name, transaction_id, object, args, .. } = message else {
            return Err(TestCaseError::fail("expected command message"));
        };

        prop_assert_eq!(name.as_str(), "publish");

        let decoded = RtmpCommand::from_message(&name, transaction_id, object, args)
            .expect("publish should decode");
        match decoded {
            RtmpCommand::Publish(cmd) => {
                prop_assert_eq!(cmd.transaction_id, transaction_id);
                prop_assert_eq!(cmd.stream_name, stream_name);
            }
            _ => return Err(TestCaseError::fail("expected publish command")),
        }
    }

    /// Play の roundtrip を検証
    #[test]
    fn play_roundtrip(
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
        start in prop::num::f64::NORMAL,
    ) {
        let command = RtmpCommand::Play(shiguredo_rtmp::tests::RtmpPlayCommand {
            transaction_id,
            stream_name: stream_name.clone(),
            start,
        });

        let message = command.into_pcm_message().expect("play should encode");
        let RtmpMessage::Command { name, transaction_id, object, args, .. } = message else {
            return Err(TestCaseError::fail("expected command message"));
        };

        prop_assert_eq!(name.as_str(), "play");

        let decoded = RtmpCommand::from_message(&name, transaction_id, object, args)
            .expect("play should decode");
        match decoded {
            RtmpCommand::Play(cmd) => {
                prop_assert_eq!(cmd.transaction_id, transaction_id);
                prop_assert_eq!(cmd.stream_name, stream_name);
                prop_assert_eq!(cmd.start, start);
            }
            _ => return Err(TestCaseError::fail("expected play command")),
        }
    }

    /// DeleteStream の roundtrip を検証
    #[test]
    fn delete_stream_roundtrip(
        transaction_id in arb_transaction_id(),
        stream_id in arb_stream_id(),
    ) {
        let command = RtmpCommand::DeleteStream(shiguredo_rtmp::tests::RtmpDeleteStreamCommand {
            transaction_id,
            stream_id,
        });

        let message = command.into_pcm_message().expect("deleteStream should encode");
        let RtmpMessage::Command { name, transaction_id, object, args, .. } = message else {
            return Err(TestCaseError::fail("expected command message"));
        };

        prop_assert_eq!(name.as_str(), "deleteStream");

        let decoded = RtmpCommand::from_message(&name, transaction_id, object, args)
            .expect("deleteStream should decode");
        match decoded {
            RtmpCommand::DeleteStream(cmd) => {
                prop_assert_eq!(cmd.transaction_id, transaction_id);
                prop_assert_eq!(cmd.stream_id, stream_id);
            }
            _ => return Err(TestCaseError::fail("expected deleteStream command")),
        }
    }

    /// GetStreamLength の roundtrip を検証
    #[test]
    fn get_stream_length_roundtrip(
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
    ) {
        let command = RtmpCommand::GetStreamLength(shiguredo_rtmp::tests::RtmpGetStreamLengthCommand {
            transaction_id,
            stream_name: stream_name.clone(),
        });

        let message = command.into_pcm_message().expect("getStreamLength should encode");
        let RtmpMessage::Command { name, transaction_id, object, args, .. } = message else {
            return Err(TestCaseError::fail("expected command message"));
        };

        prop_assert_eq!(name.as_str(), "getStreamLength");

        let decoded = RtmpCommand::from_message(&name, transaction_id, object, args)
            .expect("getStreamLength should decode");
        match decoded {
            RtmpCommand::GetStreamLength(cmd) => {
                prop_assert_eq!(cmd.transaction_id, transaction_id);
                prop_assert_eq!(cmd.stream_name, stream_name);
            }
            _ => return Err(TestCaseError::fail("expected getStreamLength command")),
        }
    }

    /// _result の roundtrip を検証
    #[test]
    fn result_roundtrip(
        transaction_id in arb_transaction_id(),
        properties in arb_amf_value(),
        information in arb_amf_value(),
    ) {
        let command = RtmpCommand::Result(shiguredo_rtmp::tests::RtmpResultCommand {
            transaction_id,
            properties: properties.clone(),
            information: information.clone(),
        });

        let message = command.into_pcm_message().expect("_result should encode");
        let RtmpMessage::Command { name, transaction_id, object, args, .. } = message else {
            return Err(TestCaseError::fail("expected command message"));
        };

        prop_assert_eq!(name.as_str(), "_result");

        let decoded = RtmpCommand::from_message(&name, transaction_id, object, args)
            .expect("_result should decode");
        match decoded {
            RtmpCommand::Result(cmd) => {
                prop_assert_eq!(cmd.transaction_id, transaction_id);
                prop_assert_eq!(cmd.properties, properties);
                prop_assert_eq!(cmd.information, information);
            }
            _ => return Err(TestCaseError::fail("expected _result command")),
        }
    }

    #[test]
    fn connect_invalid_transaction_id_rejected(
        transaction_id in (0i64..=10i64).prop_filter("not connect id", |v| *v != 1),
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
    ) {
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
        prop_assert!(result.is_err());
    }

    #[test]
    fn publish_missing_args_rejected(transaction_id in arb_transaction_id()) {
        let result = RtmpCommand::from_message(
            "publish",
            transaction_id,
            AmfValue::Amf0(Amf0Value::Null),
            vec![],
        );
        prop_assert!(result.is_err());
    }

    #[test]
    fn publish_invalid_type_rejected(
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
        publish_type in arb_small_string(),
    ) {
        prop_assume!(publish_type != "live");
        let result = RtmpCommand::from_message(
            "publish",
            transaction_id,
            AmfValue::Amf0(Amf0Value::Null),
            vec![
                AmfValue::Amf0(Amf0Value::String(stream_name)),
                AmfValue::Amf0(Amf0Value::String(publish_type)),
            ],
        );
        prop_assert!(result.is_err());
    }

    #[test]
    fn play_missing_start_rejected(
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
    ) {
        let result = RtmpCommand::from_message(
            "play",
            transaction_id,
            AmfValue::Amf0(Amf0Value::Null),
            vec![AmfValue::Amf0(Amf0Value::String(stream_name))],
        );
        prop_assert!(result.is_err());
    }

    #[test]
    fn play_invalid_start_type_rejected(
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
    ) {
        let result = RtmpCommand::from_message(
            "play",
            transaction_id,
            AmfValue::Amf0(Amf0Value::Null),
            vec![
                AmfValue::Amf0(Amf0Value::String(stream_name)),
                AmfValue::Amf0(Amf0Value::Null),
            ],
        );
        prop_assert!(result.is_err());
    }

    #[test]
    fn delete_stream_missing_id_rejected(transaction_id in arb_transaction_id()) {
        let result = RtmpCommand::from_message(
            "deleteStream",
            transaction_id,
            AmfValue::Amf0(Amf0Value::Null),
            vec![],
        );
        prop_assert!(result.is_err());
    }
}
