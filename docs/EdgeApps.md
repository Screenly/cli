# Edge Apps

Edge Apps is a framework for building and running content for your digital signage screens. Inspired by Serverless movement, Edge Apps enables you to build sophisticated digital signage content without the need for running your own infrastructure.

If you're familiar with Heroku, Cloudflare Workers, or any similar technology, you'll feel right at home with Edge Apps.

## Table of Contents

Here's a content section for the document:

1. [Getting Started](#getting-started)
   - [Create an Edge App](#create-an-edge-app)
   - [Playground Edge Apps](#playground-edge-apps)
   - [Deploy the Edge App](#deploy-the-edge-app)
   - [Instances](#instances)
     - [Instance Manifest](#instance-manifest)
     - [Instance Manifest Reference](#instance-manifest-reference)
2. [Manifest File](#manifest-file)
   - [Reference](#reference)
     - [Syntax](#syntax)
     - [ID](#id)
     - [Entrypoint](#entrypoint)
     - [Description](#description)
     - [Icon](#icon)
     - [Author](#author)
     - [Homepage URL](#homepage-url)
     - [Auth](#auth)
     - [Ready Signal](#ready-signal)
     - [Settings](#settings)
   - [Settings](#settings)
     - [Defining a Setting](#defining-a-setting)
     - [Setting a Setting](#setting-a-setting)
     - [Modify the Greeting](#modify-the-greeting)
     - [Getting Settings](#getting-settings)
     - [Using Settings in Your Edge App](#using-settings-in-your-edge-app)
     - [Global Settings](#global-settings)
     - [Secret Settings](#secret-settings)
     - [Reserved Setting Names](#reserved-setting-names)
3. [Global Branding Settings](#global-branding-settings)
   - [Branding Settings List](#branding-settings-list)
     - [screenly_color_accent](#screenly_color_accent)
     - [screenly_color_light](#screenly_color_light)
     - [screenly_logo_light](#screenly_logo_light)
   - [Notes on Branding Settings](#notes-on-branding-settings)
4. [Security Model](#security-model)
   - [Transmission and Storage Security](#transmission-and-storage-security)
5. [Metadata](#metadata)
6. [Edge App Emulator](#edge-app-emulator)
   - [Run Edge App Emulator](#run-edge-app-emulator)
   - [Add Mock Data to Run Edge App Emulator](#add-mock-data-to-run-edge-app-emulator)
7. [Debugging](#debugging)
8. [Cross-Origin Resource Sharing (CORS)](#cross-origin-resource-sharing-cors)
9. [Monitoring](#monitoring)
10. [Gotchas](#gotchas)
11. [Runtime Environment](#runtime-environment)
12. [Browser Sandboxing](#browser-sandboxing)
13. [Isolated Runtime Environment](#isolated-runtime-environment)

## Getting Started

First, you need to install the Screenly [CLI](https://developer.screenly.io/cli). The CLI is used to both generate and upload Edge Apps.

With the CLI installed and logged in (`screenly login`), we can create our "Hello World" example.

To do this, first create a new directory where the Edge App will reside. For production applications, this would likely be a source-controlled folder (e.g., `git`), but for now, let's create a temporary folder with:

```shell
$ mkdir -p ~/tmp/edge-app
$ cd ~/tmp/edge-app
```

### Create an Edge App

> To create an Edge App, simply run:

```shell
$ screenly edge-app create --name hello-world
```

When you run the screenly edge-app create command, two files will be created in the current directory:

- `screenly.yml`
- `index.html`

`screenly.yml` contains the metadata. In this file, you can define settings, secrets, and various other metadata. In our "Hello World" example, we have a single setting called `greeting`, which is used in the Edge App.

`index.html` is our entry point. It is what the client (i.e., the player) will load. This particular file is very simple and just includes some styling and various metadata examples.

Once you have initiated your Edge App, you can start adding content. We make a few assumptions about your Edge App:

* No files below the current can be references (just like you can't use the `COPY` in a `Dockerfile` outside the current working directory)
* The following file names are **reserved** by the system:
  * `screenly.yml` - Reserved for the manifest file.
  * `screenly.js` - Reserved for on-device usage to interact with the system.

Other than that, you can develop your Edge App just like you would do with a regular static HTML site. You can break out JavaScript, CSS, images, etc., into separate files and include them as you normally would.

---

#### Playground Edge Apps

Getting started with our existing Playground Edge Apps can help ease your introduction to Edge Apps development. To test your skills, first clone our Playground GitHub Repository (https://github.com/Screenly/Playground). After cloning, navigate to one of the example Playground Edge App folders and execute the following command:

For instance, if our target is the Clock App, enter the directory (`Playground/edge-apps/clock`) and execute:

```shell
$ screenly edge-app create --name "My Groundbreaking Clock App" --in-place
```

Note the `--in-place` parameter. This is necessary when creating an app with existing `screenly.yml` and `index.html` files, as our Playground Edge Apps do. Otherwise, you'll encounter errors about conflicting files. This parameter is not mandatory if you are creating a brand new Edge App; it’s just here to make your developer life a little bit easier.

---

### Deploy the Edge App

To deploy an Edge App, use the following command:

```shell
$ screenly edge-app deploy
```

The deployment process includes the following steps:

**Upload:** The Edge App is uploaded to the server.


**Replacement:** If the Edge App has been previously uploaded, the existing files are replaced with the new ones.


**Sync Settings:** The deployment synchronizes the settings from the manifest file with the server.


**Automatic Update:** All instances of the Edge App will automatically update to the latest deployed version, ensuring consistency across all devices.


---
### Instances

An instance is a unique installation of an Edge App. Each instance can have its own settings and secrets, allowing you to run multiple instances of the same Edge App with different configurations. While the CLI currently supports managing a single instance via the `instance.yml` file, the Screenly Web Dashboard can manage multiple instances simultaneously.

Each instance produces an asset that can be scheduled on a screen. This is the only method for scheduling an Edge App on a screen.

To manage instances, you can use the following commands:

```shell
$ screenly edge-app instance list
$ screenly edge-app instance create
$ screenly edge-app instance update
$ screenly edge-app instance delete
```

- **Create**: The `create` command generates a new instance and creates an `instance.yml` file in the current directory.


- **List**: The `list` command displays all instances associated with your account.


- **Update**: The `update` command modifies an existing instance based on the changes made in the `instance.yml` file.


- **Delete**: The `delete` command removes an instance from your account.


After creating an instance, navigate to your Screenly web console. You should see the new instance of your Edge App listed in the content section, where you can schedule it as you would with a regular asset.

---

#### Instance Manifest

The `instance.yml` file is used to manage the instance of an Edge App. Currently, the CLI supports managing only one instance at a time.

#### Instance Manifest Reference

##### Syntax

The `syntax` field specifies the version of the instance manifest file. The current version is `instance_v1`.

##### ID

The `id` field is a unique identifier for the instance. This ID is generated by the system and is used to identify the instance across the platform.

##### Name

The `name` field specifies the name of the instance. It can be updated by modifying the value in the `instance.yml` file and executing the `screenly edge-app instance update` command.

##### Entrypoint URI

The `entrypoint_uri` field specifies the entry point for the Edge App. This field is only required when the Edge App entry point is a remote URL. It can be updated by modifying the value in the `instance.yml` file and executing the `screenly edge-app instance update` command.

---

## Manifest File

The manifest file defines various properties of an Edge App and is loosely inspired by the [Chrome extension manifest file](https://developer.chrome.com/docs/extensions/mv3/manifest/), but uses YAML (specifically [StrictYAML](https://hitchdev.com/strictyaml/)).

When you create a new Edge App using the CLI (`screenly edge-app create --name <APP NAME>`), a manifest file named `screenly.yaml` is automatically created in the current directory.

> Manifest Reference

```yaml
syntax: manifest_v1
id: 01H7DD8SV32F9FKWXXXXXXXXXX
entrypoint:
  type: file
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

---
### Reference

#### Syntax

The `syntax` field specifies the version of the manifest file. The current version is `manifest_v1`.

#### ID

The `id` field is a unique identifier for the Edge App. This ID is generated by the system and is used to identify the Edge App across the platform.

#### Entrypoint

The `entrypoint` field specifies the entry point for the Edge App. It is optional and defaults to the file type. The `entrypoint` field contains the following subfields:
- **type:** Defines the type of entry point. Possible values are:
  - `file`: The entry point is the `index.html` file in the Edge App directory.
  - `remote_global`: The entry point is a URL that is consistent across all instances of the Edge App.
  - `remote_local`: The entry point is a URL unique to each instance of the Edge App.
- **uri:** The URL of the entry point. This field is required if the type is `remote_global`.

#### Description

The `description` field provides a brief overview of the Edge App, which will be visible in the Screenly web dashboard.

#### Icon

The `icon` field is a URL pointing to an image that represents the Edge App. This image will be displayed in the Screenly web dashboard.

#### Author

The `author` field specifies the name of the Edge App's creator.

#### Homepage URL

The `homepage_url` field is a URL directing to the homepage of the Edge App.

#### Auth

The `auth` field is optional and is used to configure Edge App authentication. It includes the following subfields:
- **auth_type:** Specifies the type of authentication. Possible values are:
  - `basic`: Basic authentication.
  - `bearer`: Bearer token authentication.
- **global:** A boolean value that determines whether the authentication is global (`true`) or local (`false`).

For Basic authentication, the following settings are generated:
- `screenly_basic_auth_username`
- `screenly_basic_auth_password`

For Bearer authentication, the following setting is generated:
- `screenly_bearer_token`

These settings must be configured using the `screenly edge-app setting set` command.

#### Ready Signal

The `ready_signal` field is an optional boolean parameter in the Edge App configuration.

##### Purpose

When set to `true`, it enables a mechanism for the Edge App to control when it's displayed on the Screenly Player.

##### Use Case

This feature is particularly useful for apps that require initial loading or data preparation before they're ready to be shown. For example, an app might need to fetch data or complete some initialization process.

##### Implementation

When the Edge App is prepared and ready to be displayed, it must call the `screenly.signalReadyForRendering()` function. This signals to the Screenly Player that the app is now ready for rendering.

##### Important Notes

1. If `ready_signal` is `true` and the function is never called, the content will not be displayed.
2. Once the function has been called successfully, any subsequent calls will be ignored.
3. If `ready_signal` is `false`, the content will be displayed as soon as possible, and any calls to `screenly.signalReadyForRendering()` will have no effect.

##### Benefit

This mechanism allows for a smoother user experience by ensuring that the Edge App is fully prepared before it becomes visible on the display.

#### Settings

The `settings` field is a dictionary of key-value pairs that define the configurable settings for the Edge App. For more details, refer to the [Settings](#settings) section.
---

### Settings

#### Defining a Setting

To define a setting, you can use the following structure in your manifest file:

```yaml
settings:
  [...]
  greeting:
    type: string
    default_value: "Cowboy Neil"
    title: greeting
    optional: true
    help_text: An example of a string setting that is used in index.html
```

### Setting a Setting

### Modify the Greeting

With the asset scheduled on your screen, you should see the headline "Hello Stranger!". This is actually a setting configured in `screenly.yml`. You can override this using the `edge-app setting` command to change it.

```shell
$ screenly edge-app setting set greeting='Cowboy Neil'
```

This will output:

```
Edge app setting successfully set.
```

It might take a few minutes for your screen to pick up the change, but once it does, the headline should change from "Hello Stranger!" to "Hello Cowboy Neil!".



#### Getting Settings

To list the current settings, use the following command:

```shell
$ screenly edge-app setting list
```

This will output:

```
+----------+-------------+---------------+----------+--------+-----------------------------------------------------------+
| Title    | Value       | Default value | Optional | Type   | Help text                                                 |
+----------+-------------+---------------+----------+--------+-----------------------------------------------------------+
| greeting | Cowboy John | Cowboy Neil   | Yes      | string | An example of a string setting that is used in index.html |
+----------+-------------+---------------+----------+--------+-----------------------------------------------------------+
```

#### Using Settings in Your Edge App

To use the settings in your Edge App, include them in your HTML or JavaScript files as follows:

```html
[...]
<head>
<script src="screenly.js?version=1"></script>
</head>
[...]
<script>
    document.getElementById("greeting").innerText = screenly.settings.greeting;
</script>
[...]
```

Settings are key-value pairs that users installing the app must provide at install time and can later edit.

---

#### Global Settings

Global settings are secrets defined for the instances of an Edge App. These settings are shared across all instances of the app and are specified in the manifest file.

```yaml
settings:
  [...]
  global_greeting:
    type: string
    default_value: "Cowboy Neil"
    title: greeting
    optional: true
    help_text: An example of a string setting that is used in index.html
    global: true
```

#### Secret settings

Secret settings are similar to regular settings but are used for sensitive data that should not be exposed to the user. These settings are stored securely and are not visible to the user.
Secrets settings must not have a default_value.

```yaml
settings:
  [...]
  api_key:
    type: secret
    title: API Key
    optional: false
    help_text: An example of an API key
```

#### Reserved setting names

Settings starting with `screenly_` are reserved and cannot be used in the manifest file.

---
## Global Branding Settings

The Global Branding Settings feature automatically provides relevant visual identity information by fetching it from the public sources based on their email domain. This functionality is available by default for every Edge App, requiring no special actions from the developer to access the following settings:

### Branding Settings List
* screenly_color_accent
* screenly_color_light
* screenly_logo_light

#### screenly_color_accent

This is the accent color of the customer's brand.

| Setting               | Example Value |
|-----------------------|----------------|
| screenly_color_accent | #972eff         |

#### screenly_color_light

This is the base color for the light theme.

| Setting              | Example Value |
|----------------------|----------------|
| screenly_color_light | #adafbe        |

#### screenly_logo_light

This is the company logo for the light theme.

| Setting             | Example Value                                                        |
|---------------------|----------------------------------------------------------------------|
| screenly_logo_light | [screenly_logo_light](https://us-assets.screenlyapp.com/1dp2TKWzR0BCB38bI170Xf) |

### Notes on Branding Settings

These settings are available to the Edge App like the usual settings but cannot be listed, fetched, or changed with CLI.

---

### Security Model

Screenly's secrets function similarly to settings but with enhanced security measures. They are write-only, meaning they cannot be retrieved via the API or web interface once written.

Secrets are defined in `screenly.yml`, but their values are not set within the manifest file. Instead, they are securely managed through the Edge App.

#### Transmission and Storage Security

  * **Screenly Player Max**: Each device has a unique pair of public/private keys stored in a Trusted Platform Module (TPM), allowing the use of x509 cryptography. Payloads sent to a Screenly Player are encrypted with the device's public key, ensuring only the intended device can decrypt it. Secrets are encrypted on disk using the TPM, making them inaccessible even if the hard drive is compromised.

  * **Standard Screenly Player**: While these devices do not have a TPM, they still use x509 cryptography with certificates securely stored on disk. This provides a high level of protection for stored secrets, even without hardware-level security.

Secrets ensure that sensitive data is securely managed and transmitted, providing robust security for your applications.

---

## Metadata

> Sample Edge App Use

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <title>Metadata</title>
    <script src="screenly.js?version=1"></script>
  </head>
  <body >
      <p>I'm <span id="screen-name"></span>.</p>
  </body>
  <script>
    document.getElementById("screen-name").innerText = screenly.metadata.screen_name;
  </script>
</html>
```

In many scenarios, you may need to utilize metadata within your Edge Apps. For example, you might want to develop a wayfinding app that shows the location of the screen and directs users to a specific destination.

Since each screen inherently possesses a wealth of information about itself, including its geographical location, we've made this data accessible within the Edge App framework.

This metadata includes:

* **name:** The human-readable name assigned to the screen.
* **hostname:** The unique hostname (which also serves as the identifier) assigned to the screen.
* **coordinates:** The latitude and longitude coordinates of the screen's location, which can be edited via the web interface.
  * Returned as a dictionary.
* **location:** The human-readable location as displayed in the web interface.
* **hardware:** Details about the hardware of the device.
* **version:** The software version running on the Screenly device.
* **tags:** The tags or labels assigned to the screen.
  * Returned as a dictionary.

---

## Edge App emulator

After creating your Edge App, you can use the Edge App emulator to test it in your web browser.

* To do this, open your terminal and navigate to the Edge App directory.
* Run this command:

> Run Edge App emulator

```shell
$ screenly edge-app run
```

This command will provide you with a URL to access your Edge App in your browser.

If you don't have sample data in your Edge App directory, you can create it by running this command:

> Add mock data to run Edge App emulator

```shell
$ screenly edge-app run --generate-mock-data
```

After generating the mock data, run the Edge App emulator again to see your app in action.

---

## Debugging

Coming soon.

---

## Cross-Origin Resource Sharing (CORS)

Cross-origin resource sharing is a mechanism that allows restricted resources on a web page to be accessed from another domain outside the domain from which the first resource was served. Some APIs, particularly public ones, use CORS to restrict access. Sometimes you need to bypass CORS. To do this, we provide you with a handy CORS proxy mitigation strategy.

The way it works is very straightforward. Instead of accessing the API directly from JavaScript, you access it via the CORS proxy. The CORS proxy removes the CORS policy so that you can circumvent the restriction.

For instance, if you're trying to access the API endpoint `https://api.example.com/v1`, but it has a CORS policy preventing access, you can bypass this policy using the CORS proxy built into Edge Apps. Here's how you can modify your code to use the CORS proxy:

```html
[...]
<head>
<script src="screenly.js?version=1"></script>
</head>
<body>
  [...]
  <script>
    cost api_url = 'https://api.example.com/v1';
    cost bypass_cors_url = screenly.cors_proxy_url + api_url;
  </script>
  [...]
</body>
```

In the example code snippet above, replace `api_url` with `bypass_cors_url` to interface with the API as usual through the CORS proxy.

---

## Monitoring

*Monitoring is in invite-only beta.*

When developing Edge Apps, it's crucial to monitor the performance of the device running the app. This is especially important for low-powered devices, such as Raspberry Pis, where resources are limited.

To assist with monitoring, we have chosen [Prometheus](https://prometheus.io) as the platform to expose metrics.

When monitoring is enabled, the device exposes Prometheus metrics at port `9100`, utilizing [Node Exporter](https://prometheus.io/docs/guides/node-exporter/#monitoring-linux-host-metrics-with-the-node-exporter). This enables you to scrape metrics and visualize them using tools like [Grafana](https://grafana.com/), which can be integrated seamlessly with Screenly for visualization purposes ([learn more about Grafana integration with Screenly](https://www.screenly.io/tutorials/grafana/)).

---

## Gotchas

* You need to use **relative** paths to assets (e.g., static/image.svg rather than absolute paths (e.g., /static/image.svg).

---

## Runtime Environment

Security is at the core of Edge Apps, and several measures have been implemented to ensure their security.

### Browser Sandboxing

Edge Apps run inside the browser within appropriate security profiles, ensuring a sandboxed environment.

### Isolated Runtime Environment

The Edge App runtime environment itself is isolated, preventing Edge Apps from communicating or accessing files/data from another Edge App. This isolation extends not only to files and environment data (such as settings) but also to secrets.
