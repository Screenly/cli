## Edge Apps Versions

You can list all Edge Apps in a given account, along with their versions with the `edge-app version list` command. This will also tell you what version is the current version.

> List Edge Apps (and versions)

```bash
screenly edge-app version list
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
screenly edge-app version promote \
    --revision=1 \
    --channel=candidate
```
```
Promote 1 of Edge App 'Weather App' (XXXXXXXXXXXXXXXXXXXXXXXXX)? (y/n)
```

> Promote to latest version

```shell
edge-app version promote --latest
```

> Delete a version

```shell
screenly edge-app version delete v2
```
```
Delete v2 of Edge App 'Weather App' (XXXXXXXXXXXXXXXXXXXXXXXXX)? (y/n)
```
