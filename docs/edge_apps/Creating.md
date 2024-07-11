## Creating an Edge App

To create an Edge App, you need to use the CLI and invoke it using `edge-app create <name>`. This will fire off a number of API calls and create a file called `screenly.yml` in the current directory, as well as a sample `index.html` file.

> To create an Edge App, simply run:
```shell
screenly edge-app create --name <name>
```

Once you have initiated your Edge App, you can start adding content. We make a few assumptions about your Edge App:

* No files below the current can be references (just like you can't use the `COPY` in a `Dockerfile` outside the current working directory)
* The following file names are **reserved** by the system:
  * `screenly.yml` - Reserved for the manifest file.
  * `screenly.js` - Reserved for on-device usage to interact with the system.

Other than that, you can develop your Edge App just like you would do with a regular static HTML site. You can break out JavaScript, CSS, images, etc., into separate files and include them as you normally would.
