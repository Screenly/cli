## Getting Started

First, you need to install the Screenly [CLI](/cli). The CLI is used to both generate and upload Edge Apps.

With the CLI installed and logged in (`screenly login`), we can create our "Hello World" example.

To do this, first create a new directory where the Edge App will reside. For production applications, this would likely be a source-controlled folder (e.g., `git`), but for now, let's create a temporary folder with:

```shell
mkdir -p ~/tmp/edge-app
cd ~/tmp/edge-app
```

### Create an Edge App

```shell
screenly edge-app create --name hello-world
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
screenly edge-app create --name "My Groundbreaking Clock App" --in-place
```

Note the `--in-place` parameter. This is necessary when creating an app with existing `screenly.yml` and `index.html` files, as our Playground Edge Apps do. Otherwise, you'll encounter errors about conflicting files. This parameter is not mandatory if you are creating a brand new Edge App; it’s just here to make your developer life a little bit easier.

### Upload the Edge App

```shell
screenly edge-app upload
```

To use this Edge App, first upload it using the upload command. This will automatically create a new version (you can see your versions using screenly edge-app version list). After the Edge App is successfully uploaded, promote it to a channel (stable or candidate) to use it on the player.

#### List Edge App Versions

```shell
screenly edge-app version list
```

```
+----------+-------------------------+-----------+----------+
| Revision | Description             | Published | Channels |
+----------+-------------------------+-----------+----------+
| 1        | Screenly Clock Edge App | ✅        |          |
+----------+-------------------------+-----------+----------+
```

### Promote the Edge App

The `upload` command only uploads the Edge App and its assets to the server. To make it available for screens, you need to promote it. This way, it will be available for further processing.
```shell
screenly edge-app version promote --latest
```

```
Edge app version successfully promoted.
```

Once you have promoted a release, you can start using it. If you head over to your Screenly web console, you should see your Edge App listed. Schedule it as you would with a regular asset.

With the asset scheduled on your screen, you should see the headline "Hello Stranger!". This is actually a setting configured in `screenly.yml`. You can override this using the `edge-app setting` command to change it.

### Modify the Greeting

```bash
screenly edge-app setting set greeting='Cowboy Neil'
```

It might take a few minutes for your screen to pick up the change, but once it does, the headline should change from "Hello Stranger!" to "Hello Cowboy Neil!".
