use serde::{Deserialize, Serialize};

use crate::commands::serde_utils::{
    deserialize_option_string_field, string_field_is_none_or_empty,
};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceManifest {
    #[serde(
        deserialize_with = "deserialize_instance_id",
        skip_serializing_if = "string_field_is_none_or_empty",
        default
    )]
    pub instance_id: Option<String>,
}

fn deserialize_instance_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let maybe_instance_id = deserialize_option_string_field("app_id", true, deserializer);

    maybe_instance_id.map_err(|_e| {
        serde::de::Error::custom("Enter a valid ULID `instance_id` parameter either in the maniphest file or as a command line parameter (e.g. `--app_id XXXXXXXXXXXXXXXX`). Field \"app_id\" cannot be empty in the maniphest file (instance.yml)")
    })
}
