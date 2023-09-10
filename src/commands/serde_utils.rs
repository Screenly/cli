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
                Err(serde::de::Error::custom(format!("Field \"{}\" cannot be empty", field_name)))
            } else {
                Ok(None)
            }
        }
        _ => Ok(opt),
    }
}

pub fn string_field_is_none_or_empty(opt: &Option<String>) -> bool {
    opt.as_ref().map_or(true, |s| s.is_empty())
}