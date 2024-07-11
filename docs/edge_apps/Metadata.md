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

There are numerous scenario where you want to use some sort of metadata for your Edge Apps. For instance, you might want to build a wayfinding apps that shows the location from where the screen is located to a particular destination.

Since the screen already knows a lot about itself, including where it is in the world, we've exposed this to the Edge App framework.

This data includes:

* **name:** The human readable name you have given the screen.
* **hostname:** The unique hostname (which also corresponds with the id) the screen has been given.
* **coordinates:** The latitude and longitude coordinates where the screen has been set to. You can edit this location in the web interface.
  * Returned as a dict.
* **location:** The human readable location as shown in the web interface.
* **hardware:** The hardware for the given device.
* **version:** The software version of the Screenly device.
* **tags:** The tags/labels assigned to the screen.
  * Returned as a dict.
