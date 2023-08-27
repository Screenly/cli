#!/bin/bash
# Validate YAML against JSON Schema.

# Exit codes
SUCCESS=0
ERR_USAGE=1
ERR_VALIDATION_FAILED=2

if [[ -z "$1" ]]; then
  echo "Usage: $0 <PATH_TO_YAML_FILE>"
  exit $ERR_USAGE
fi

SCHEMA_PATH="./schema/screenly_yml_schema.json"

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
    exit($ERR_VALIDATION_FAILED)
else:
    print("Validation successful!")
    exit($SUCCESS)
END

PYTHON_EXIT_CODE=$?

exit $PYTHON_EXIT_CODE
