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
