use serde::{Deserialize, Deserializer};

pub fn indent_yaml_sequences(yaml: &str) -> String {
    let mut result = String::with_capacity(yaml.len() + 32);
    let mut lines = yaml.lines().peekable();

    while let Some(line) = lines.next() {
        result.push_str(line);
        result.push('\n');

        let trimmed = line.trim_end();
        if !trimmed.ends_with(':') {
            continue;
        }

        let key_indent = line.len() - line.trim_start().len();
        let extra = "  ";

        while let Some(&next) = lines.peek() {
            let next_trimmed = next.trim_start();
            let next_indent = next.len() - next_trimmed.len();
            if (next_trimmed.starts_with("- ") || next_trimmed == "-") && next_indent == key_indent
            {
                lines.next();
                result.push_str(extra);
                result.push_str(next);
                result.push('\n');

                while let Some(&cont) = lines.peek() {
                    let cont_trimmed = cont.trim_start();
                    let cont_indent = cont.len() - cont_trimmed.len();
                    if cont_indent > key_indent {
                        lines.next();
                        result.push_str(extra);
                        result.push_str(cont);
                        result.push('\n');
                    } else {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    result
}

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
                    "Field \"{field_name}\" cannot be empty"
                )))
            } else {
                Ok(None)
            }
        }
        _ => Ok(opt),
    }
}

pub fn string_field_is_none_or_empty(opt: &Option<String>) -> bool {
    match opt.as_ref() {
        None => true,
        Some(s) => s.is_empty(),
    }
}
