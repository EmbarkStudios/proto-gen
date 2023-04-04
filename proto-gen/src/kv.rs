use clap::builder::TypedValueParser;
use clap::error::ContextKind;
use clap::error::ContextValue;

#[derive(Clone, Default)]
pub(crate) struct KvValueParser;

impl TypedValueParser for KvValueParser {
    type Value = (String, String);

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let str_value = value.to_str().ok_or_else(|| {
            let mut e = clap::Error::new(clap::error::ErrorKind::ValueValidation);
            if let Some(arg) = arg {
                e.insert(
                    ContextKind::InvalidArg,
                    ContextValue::String(arg.to_string()),
                );
            }
            e.insert(
                ContextKind::Usage,
                ContextValue::StyledStr(
                    "Both key and value of KV-pair must be valid UTF-8."
                        .to_owned()
                        .into(),
                ),
            );

            e.insert(ContextKind::InvalidValue, ContextValue::None);
            e
        })?;

        if let Some((k, v)) = str_value.split_once(':') {
            Ok((k.to_owned(), v.to_owned()))
        } else {
            let mut e = clap::Error::new(clap::error::ErrorKind::ValueValidation);
            if let Some(arg) = arg {
                e.insert(
                    ContextKind::InvalidArg,
                    ContextValue::String(arg.to_string()),
                );
            }

            e.insert(
                ContextKind::InvalidValue,
                ContextValue::String(str_value.to_owned()),
            );

            e.insert(
                ContextKind::Usage,
                ContextValue::StyledStr("KV-pair must contain at least one `:`.".to_owned().into()),
            );
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::kv::KvValueParser;
    use clap::builder::TypedValueParser;
    use clap::error::{ContextKind, ErrorKind};
    use clap::{Arg, Command};
    use std::collections::HashSet;
    use std::ffi::OsStr;

    #[test]
    fn happy_case_kv() {
        let cmd = Command::new("any");
        let a = None;
        let value = OsStr::new("key:value");
        let (key, value) = KvValueParser::default().parse_ref(&cmd, a, value).unwrap();
        assert_eq!("key", &key);
        assert_eq!("value", &value);
    }

    #[test]
    fn sad_case_bad_kv_no_arg() {
        let cmd = Command::new("any");
        let a = None;
        let value = OsStr::new("key-value");
        let Err(e) = KvValueParser::default().parse_ref(&cmd, a, value) else {
            panic!("Expected error on bad kv");
        };
        assert_eq!(ErrorKind::ValueValidation, e.kind());
        let mut expect_ctx = HashSet::new();
        expect_ctx.insert(ContextKind::InvalidValue);
        expect_ctx.insert(ContextKind::Usage);
        for (kind, _ctx) in e.context() {
            assert!(expect_ctx.remove(&kind));
        }
        assert!(expect_ctx.is_empty());
    }

    #[test]
    fn sad_case_bad_kv_with_arg() {
        let cmd = Command::new("any");
        let a = Some(Arg::new("my_id"));
        let value = OsStr::new("key-value");
        let Err(e) = KvValueParser::default().parse_ref(&cmd, a.as_ref(), value) else {
            panic!("Expected error on bad kv");
        };
        assert_eq!(ErrorKind::ValueValidation, e.kind());
        let mut expect_ctx = HashSet::new();
        expect_ctx.insert(ContextKind::InvalidArg);
        expect_ctx.insert(ContextKind::InvalidValue);
        expect_ctx.insert(ContextKind::Usage);
        for (kind, _ctx) in e.context() {
            assert!(expect_ctx.remove(&kind));
        }
        assert!(expect_ctx.is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn os_str_not_utf8_no_arg() {
        use std::os::unix::prelude::OsStrExt;
        let cmd = Command::new("any");
        let a = None;
        let value = OsStr::from_bytes(b"\xc3\x28");
        let Err(e) = KvValueParser::default().parse_ref(&cmd, a, value) else {
            panic!("Expected error on bad kv");
        };
        assert_eq!(ErrorKind::ValueValidation, e.kind());
        let mut expect_ctx = HashSet::new();
        expect_ctx.insert(ContextKind::Usage);
        expect_ctx.insert(ContextKind::InvalidValue);
        for (kind, _ctx) in e.context() {
            assert!(expect_ctx.remove(&kind));
        }
        assert!(expect_ctx.is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn os_str_not_utf8_with_arg() {
        use std::os::unix::prelude::OsStrExt;
        let cmd = Command::new("any");
        let a = Some(Arg::new("my_id"));
        let value = OsStr::from_bytes(b"\xc3\x28");
        let Err(e) = KvValueParser::default().parse_ref(&cmd, a.as_ref(), value) else {
            panic!("Expected error on bad kv");
        };
        assert_eq!(ErrorKind::ValueValidation, e.kind());
        let mut expect_ctx = HashSet::new();
        expect_ctx.insert(ContextKind::Usage);
        expect_ctx.insert(ContextKind::InvalidValue);
        expect_ctx.insert(ContextKind::InvalidArg);
        for (kind, _ctx) in e.context() {
            assert!(expect_ctx.remove(&kind));
        }
        assert!(expect_ctx.is_empty());
    }
}
