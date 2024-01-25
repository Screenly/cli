use serde::{Deserialize, Deserializer};

pub fn deserialize_option_string_field<'de, D>(
    field_name: &'static str,
    error_on_empty: bool,
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;

    match &opt {
        None => Ok(None),
        Some(ref s) if s.trim().is_empty() => {
            if error_on_empty {
                Err(serde::de::Error::custom(format!(
                    "Field \"{}\" cannot be empty",
                    field_name
                )))
            } else {
                Ok(None)
            }
        }
        _ => Ok(opt),
    }
}

pub fn serialize_non_empty_string_field<S>(
    field_name: &'static str,
    value: &str,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if value.trim().is_empty() {
        Err(serde::ser::Error::custom(format!(
            "Field \"{}\" cannot be empty",
            field_name
        )))
    } else {
        serializer.serialize_str(value)
    }
}

pub fn deserialize_string_field<'de, D>(
    field_name: &'static str,
    error_on_empty: bool,
    deserializer: D,
) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;

    if s.trim().is_empty() && error_on_empty {
        Err(serde::de::Error::custom(format!(
            "Field \"{}\" cannot be empty",
            field_name
        )))
    } else {
        Ok(s)
    }
}

pub fn string_field_is_none_or_empty(opt: &Option<String>) -> bool {
    opt.as_ref().map_or(true, |s| s.is_empty())
}

pub fn deserialize_bool_field<'de, D>(
    field_name: &'static str,
    deserializer: D,
) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value: bool = true;
    // let value: bool = bool::deserialize(deserializer)?;
    Ok(value)
}
