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
