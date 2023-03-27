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
