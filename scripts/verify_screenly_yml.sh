#!/bin/bash
# Validate YAML against JSON Schema.

if [[ -z "$1" ]]; then
  echo "Usage: $0 <PATH_TO_YAML_FILE>"
  exit 1
fi

SCHEMA_PATH="./schema/screenly_yml_schema.json"

if [[ ! "$SCHEMA_PATH" = /* ]]; then
  SCHEMA_PATH="$(cd "$(dirname "$SCHEMA_PATH")" && pwd)/$(basename "$SCHEMA_PATH")"
fi

python3 - <<END
import os
import json
import yaml
from jsonschema import Draft4Validator

yaml_file_path = os.path.abspath("$1")
json_schema_path = "$SCHEMA_PATH"

with open(yaml_file_path, 'r') as f:
    yaml_data = yaml.safe_load(f)

with open(json_schema_path, 'r') as f:
    schema = json.load(f)

validator = Draft4Validator(schema)
errors = sorted(validator.iter_errors(yaml_data), key=lambda e: e.path)

if errors:
    print("Validation failed!")
    for error in errors:
        print(f"  Error at {list(error.path)}: {error.message}")
else:
    print("Validation successful!")

END
