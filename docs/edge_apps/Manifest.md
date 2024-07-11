## Manifest File

The manifest file defines various properties of an Edge App and is loosely inspired by the [Chrome extension manifest file](https://developer.chrome.com/docs/extensions/mv3/manifest/), but uses YAML (specifically [StrictYAML](https://hitchdev.com/strictyaml/)).

When you create a new Edge App using the CLI (`screenly edge-app create --name <APP NAME>`), a manifest file named `screenly.yaml` is automatically created in the current directory.

> Manifest Reference

```yaml
app_id: 01H7DD8SV32F9FKWXXXXXXXXXX
entrypoint: index.html
description: 'Displays the current weather and time'
icon: 'https://example.com/some-logo.svg'
author: 'Screenly, Inc'
homepage_url: 'https://www.screenly.io'
settings:
   google_maps_api_key:
    type: secret
    title: API Key
    optional: false
    help_text: Specify a commercial Google Maps API key. Required due to the app's map feature.
  greeting:
    type: string
    default_value: "Cowboy Neil"
    title: greeting
    optional: true
    help_text: An example of a string setting that is used in index.html
```

Edge Apps can have settings, which are key-value pairs that users installing the app must provide at install time and can later edit. The values in these settings are exposed to the app via environment variables or secrets. There are two types of settings:

* String
* Secret

Each setting may have a default value, an optional flag indicating if it's required, a human-readable title, and help text to assist users in configuring the app.
