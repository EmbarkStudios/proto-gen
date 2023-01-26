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
                clap::error::ContextKind::Usage,
                clap::error::ContextValue::StyledStr(
                    "Both key and value of KV-pair must be valid UTF-8."
                        .to_owned()
                        .into(),
                ),
            );

            e.insert(
                clap::error::ContextKind::InvalidValue,
                clap::error::ContextValue::None,
            );
            e
        })?;

        if str_value.chars().filter(|c| c == &':').count() != 1 {
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
                ContextValue::StyledStr("KV-pair must contain exactly one `:`.".to_owned().into()),
            );

            return Err(e);
        }
        let mut parts = str_value.split(':');

        // SAFETY: Unwrap OK; we know we have exactly one split, two elements.
        Ok((
            parts.next().unwrap().to_owned(),
            parts.next().unwrap().to_owned(),
        ))
    }
}
