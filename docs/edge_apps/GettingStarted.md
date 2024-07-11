## Getting Started

First, you need to install the Screenly [CLI](/cli). The CLI is used to both generate and to upload Edge Apps.

With the CLI installed, and logged in (`screenly login`), we can create our Hello World example.

To do this, we first need to create a new directory where the Edge App will live. For production applications, this would likely be a source controlled folder (e.g. `git`), but let's just create a temporary folder for now with `mkdir -p ~/tmp/edge-app` and jump into it with `cd ~/tmp/edge-app`.

> Create an Edge App

```bash
$ screenly edge-app create --name hello-world
```

When you run the `screenly edge-app create` command, two files will be created in the current directory:

* `screenly.yml`
* `index.html`

`screenly.yml` is where the metadata lives. In this file, you can define settings, secrets and various other meta data. In our Hello World example, we have a single setting called `greeting`, which is used in the Edge App.

`index.html` is our entry-point so to speak. It is what the client (i.e. the player) will load. This particular file is very simple and just includes some styling and various meta data examples.

**Playground Edge Apps**

Getting started with our existing Playground Edge Apps would help easing your introduction to our Edge Apps development. To test your abilities with them, you need to clone our [Playground GitHub Repository](https://github.com/Screenly/Playground) first. After cloning it, please get into one of the example Playground Edge App folder and execute following command:

Let's say our target is the Clock App. Enter into directory (`Playground/edge-apps/clock`) and execute the following command to create an Edge App.

```
$ screenly edge-app create --name "My Groundbreaking Clock App" --in-place
```

Please note the `--in-place` parameter. This is necessary if you are creating an app with existing `screenly.yml` and `index.html` files, like our Playground Edge Apps do. Otherwise you'll be getting errors about conflicting files. Of course it's not mandatory if you are creating a brand new Edge App. It's here just to make your already overloaded developer life a little bit easier.


> Upload the Edge App

```bash
$ screenly edge-app upload
Edge app successfully uploaded.
```
Now in order to consume this Edge App, we first need to upload the Edge App using the `upload` command. This will automatically create a new version (you can see your versions using `screenly edge-app version list`). With the Edge App successfully uploaded, you now need to promote it to a channel (stable or candidate) in order to use it in on the player.

> List Edge App versions

```bash
$ screenly edge-app version list
+----------+-------------------------+-----------+----------+
| Revision | Description             | Published | Channels |
+----------+-------------------------+-----------+----------+
| 1        | Screenly Clock Edge App | ✅        |          |
+----------+-------------------------+-----------+----------+
```

> Promote the Edge App

With the `upload` command you'd only upload the Edge App and its assets to the server. To make it available for screens you need to promote it. This way it will be available for further process.

```bash
$ screenly edge-app version promote --latest
Edge app version successfully promoted.
```

Once you have promoted a release, we can now start using this. If you head over to your Screenly web console, you should see your Edge App listed. Just schedule this as you would with a regular asset.

With the asset scheduled on your screen, you should see the headline "Hello stranger!". As it turns out, this is actually a setting (configured in `screenly.yml`). We can override this using the `edge-app setting` command to change this.

> Modify the greeting

```bash
$ screenly edge-app setting set greeting='Cowboy Neil'
```

It might take few minutes for your screen to pick up on the change, but once it has, the headline should change from "Hello Stranger!" to "Hello Cowboy Neil!".

## Sample Edge Apps

* [Clock](https://github.com/Screenly/Playground/tree/master/edge-apps/weather)
* [Weather](https://github.com/Screenly/Playground/tree/master/edge-apps/weather)
* [RSS Reader](https://github.com/Screenly/Playground/tree/master/edge-apps/rss-reader)
