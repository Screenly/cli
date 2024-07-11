## Creating an Edge App

To create an Edge App, you need to use the CLI and invoke it using `edge-app create <name>`. This will fire off a number of API calls and create a file called `screenly.yml` in the current directory (you can learn more about this file [here](/edge-apps#references)), as well as a sample `index.html` file.

> To create an Edge App, simply run:

```bash
$ screenly edge-app create --name <name>
```
Once you have initiated your Edge App, you can start adding content. We make a few assumptions about your Edge App:

* No files below the current can be references (just like you can't use the `COPY` in a `Dockerfile` outside the current working directory)
* The following file names are **reserved** by the system:
  * `screenly.yml` - Reserved for the manifest file.
  * `screenly.js` - Reserved for on-device usage to interact with the system.

Other than that, you can develop your Edge App just like you would do with a regular static HTML site. You can break out JavaScript, CSS, images etc into separate files and just include them as you normally would.


## Uploading an Edge App

To upload your Edge Apps to Screenly, you use the `edge-app upload` command. This will copy all files in the current directory and generate a release.

When you upload subsequent releases, you'll notice a few more things:

* The `revision` key in `screenly.yaml` will be bumped to a newer version.
* The new asset will automatically be added to the system as a new version.
* If you try to upload a new revision without any local changes, the upload will fail with `Failed to upload edge app: Cannot upload a new version: No changes detected.`


> Upload an edge app from the current directory:

```bash
$ screenly edge-app upload
```

## Edge Apps Versions

You can list all Edge Apps in a given account, along with their versions with the `edge-app version list` command. This will also tell you what version is the current version.

> List Edge Apps (and versions)

```bash
$ screenly edge-app version list
+----------+-------------+-----------+
| Revision | Description | Published |
+----------+-------------+-----------+
| 1        |             | ✅        |
+----------+-------------+-----------+
```

Using `version list`, you can determine what the 'Active Revision' is. This is the version that is corresponding to the asset that is showing on your screen(s).

To promote a new release, you can use the `version promote` command. This will automatically deploy the version you've specified. To roll back, you can promote the previous version.

You also have the option to use `--latest` to employ the most recent version of the app.

> Promote a version

```bash
$ screenly edge-app version promote \
    --revision=1 \
    --channel=candidate
Promote 1 of Edge App 'Weather App' (XXXXXXXXXXXXXXXXXXXXXXXXX)? (y/n)
```

> Promote to latest version

```bash
edge-app version promote --latest
```

> Delete a version

```bash
$ screenly edge-app version delete v2

Delete v2 of Edge App 'Weather App' (XXXXXXXXXXXXXXXXXXXXXXXXX)? (y/n)
```
