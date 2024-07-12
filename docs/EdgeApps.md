# Edge Apps

Edge Apps is a framework for building and running content for your digital signage screens. Inspired by Serverless movement, Edge Apps enables you to build sophisticated digital signage content without the need for running your own infrastructure.

If you're familiar with Heroku, Cloudflare Workers, or any similar technology, you'll feel right at home with Edge Apps.

## Table of Contents

* [Getting Started](#getting-started)
* [Creating](#creating-an-edge-app)
* [Uploading](#uploading-an-edge-app)
* [Versions](#edge-apps-versions)
* [Manifest](#manifest-file)
* [Settings](#settings)
* [Secrets](#secrets)
* [Metadata](#metadata)
* [Emulator](#edge-app-emulator)
* [Debugging](#debugging)
* [CORS](#cross-origin-resource-sharing-cors)
* [Monitoring](#monitoring)
* [Gotchas](#gotchas)
* [Runtime Environment](#runtime-environment)


## Getting Started

First, you need to install the Screenly [CLI](https://developer.screenly.io/cli). The CLI is used to both generate and upload Edge Apps.

With the CLI installed and logged in (`screenly login`), we can create our "Hello World" example.

To do this, first create a new directory where the Edge App will reside. For production applications, this would likely be a source-controlled folder (e.g., `git`), but for now, let's create a temporary folder with:

```shell
$ mkdir -p ~/tmp/edge-app
$ cd ~/tmp/edge-app
```

### Create an Edge App

```shell
$ screenly edge-app create --name hello-world
```

When you run the screenly edge-app create command, two files will be created in the current directory:

- `screenly.yml`
- `index.html`

`screenly.yml` contains the metadata. In this file, you can define settings, secrets, and various other metadata. In our "Hello World" example, we have a single setting called `greeting`, which is used in the Edge App.

`index.html` is our entry point. It is what the client (i.e., the player) will load. This particular file is very simple and just includes some styling and various metadata examples.

#### Playground Edge Apps

Getting started with our existing Playground Edge Apps can help ease your introduction to Edge Apps development. To test your skills, first clone our Playground GitHub Repository (https://github.com/Screenly/Playground). After cloning, navigate to one of the example Playground Edge App folders and execute the following command:

For instance, if our target is the Clock App, enter the directory (`Playground/edge-apps/clock`) and execute:

```shell
$ screenly edge-app create --name "My Groundbreaking Clock App" --in-place
```

Note the `--in-place` parameter. This is necessary when creating an app with existing `screenly.yml` and `index.html` files, as our Playground Edge Apps do. Otherwise, you'll encounter errors about conflicting files. This parameter is not mandatory if you are creating a brand new Edge App; it’s just here to make your developer life a little bit easier.

### Upload the Edge App

```shell
$ screenly edge-app upload
```

To use this Edge App, first upload it using the upload command. This will automatically create a new version (you can see your versions using screenly edge-app version list). After the Edge App is successfully uploaded, promote it to a channel (stable or candidate) to use it on the player.

### Promote the Edge App

The `upload` command only uploads the Edge App and its assets to the server. To make it available for screens, you need to promote it. This way, it will be available for further processing.
```shell
$ screenly edge-app version promote --latest
```

```
Edge app version successfully promoted.
```

Once you have promoted a release, you can start using it. If you head over to your Screenly web console, you should see your Edge App listed. Schedule it as you would with a regular asset.

With the asset scheduled on your screen, you should see the headline "Hello Stranger!". This is actually a setting configured in `screenly.yml`. You can override this using the `edge-app setting` command to change it.

### Modify the Greeting

```shell
$ screenly edge-app setting set greeting='Cowboy Neil'
```

It might take a few minutes for your screen to pick up the change, but once it does, the headline should change from "Hello Stranger!" to "Hello Cowboy Neil!".

---

## Creating an Edge App

To create an Edge App, you need to use the CLI and invoke it using `edge-app create <name>`. This will fire off a number of API calls and create a file called `screenly.yml` in the current directory, as well as a sample `index.html` file.

> To create an Edge App, simply run:
```shell
$ screenly edge-app create --name <name>
```

Once you have initiated your Edge App, you can start adding content. We make a few assumptions about your Edge App:

* No files below the current can be references (just like you can't use the `COPY` in a `Dockerfile` outside the current working directory)
* The following file names are **reserved** by the system:
  * `screenly.yml` - Reserved for the manifest file.
  * `screenly.js` - Reserved for on-device usage to interact with the system.

Other than that, you can develop your Edge App just like you would do with a regular static HTML site. You can break out JavaScript, CSS, images, etc., into separate files and include them as you normally would.

---

## Uploading an Edge App

To upload your Edge App to Screenly, you use the `edge-app upload` command. This will copy all files in the current directory and generate a release.

When you upload subsequent releases, you'll notice a few more things:

* The `revision` key in `screenly.yaml` will be bumped to a newer version.
* The new asset will automatically be added to the system as a new version.
* If you try to upload a new revision without any local changes, the upload will fail with `Failed to upload edge app: Cannot upload a new version: No changes detected.`


> Upload an edge app from the current directory:

```shell
$ screenly edge-app upload
```

---

## Edge Apps Versions

You can list all Edge Apps in a given account, along with their versions with the `edge-app version list` command. This will also tell you what version is the current version.

> List Edge Apps (and versions)

```shell
$ screenly edge-app version list
```
```
+----------+-------------+-----------+
| Revision | Description | Published |
+----------+-------------+-----------+
| 1        |             | ✅        |
+----------+-------------+-----------+
```

Using `version list`, you can determine what the 'Active Revision' is. This is the version that corresponds to the asset that is currently showing on your screen(s).

To promote a new release, you can use the `version promote` command. This will automatically deploy the version you've specified. To roll back, you can promote the previous version.

You also have the option to use `--latest` to employ the most recent version of the app.

> Promote a version

```shell
$ screenly edge-app version promote \
    --revision=1 \
    --channel=candidate
```
```
Promote 1 of Edge App 'Weather App' (XXXXXXXXXXXXXXXXXXXXXXXXX)? (y/n)
```

> Promote to latest version

```shell
$ edge-app version promote --latest
```

> Delete a version

```shell
$ screenly edge-app version delete v2
```
```
Delete v2 of Edge App 'Weather App' (XXXXXXXXXXXXXXXXXXXXXXXXX)? (y/n)
```

---

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

---

## Settings

Coming soon.

---

## Secrets

> Defining a secret

```yaml
settings:
  [...]
  api_key:
    type: secret
    title: API Key
    optional: false
    help_text: An example of an API key
```
> Setting a secret

```shell
$ screenly edge-app secret set api_key='ABC123'
```
Screenly's secrets function similarly to settings, but with a distinct security model. They are write-only, ensuring they can't be retrieved via the API or web interface once written. To use secrets, you define them in `screenly.yml`, but you do not set a value.

From a consumption perspective (i.e. to use them on the device), secrets are exposed the same way as settings. Thus you can't have a secret and a setting by the same name.

The transmission and storage protocols employ enhanced security. Every Screenly device has its unique pair of public/private keys. For the Screenly Player Max, these keys are securely held in its Trusted Platform Module (TPM), which allows the use of robust x509 cryptography. When we send payload to a Screenly Player, we encrypt it using the device's unique public key, ensuring that only the intended device can decrypt it. Furthermore, secrets on the Player Max are fully encrypted on disk using the TPM, making them inaccessible even if the hard drive is compromised.

For the standard Screenly Player, which doesn't have a TPM, we still utilize robust x509 cryptography with certificates securely stored on disk. While these devices do not offer hardware-level security for stored secrets, our encryption still ensures a high level of protection for your sensitive data.


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
