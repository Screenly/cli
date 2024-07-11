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
