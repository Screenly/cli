## Cross-Origin Resource Sharing (CORS)

Cross-origin resource sharing is a mechanism that allows restricted resources on a web page to be accessed from another domain outside the domain from which the first resource was served. Some APIs (particularly public ones) use CORS to restrict access. Sometimes you need to bypass CORS. To do this, we provide you with a handy CORS proxy mitigation strategy. The way it works is very straight forward. Instead of accessing the API directly from JavaScript, you instead access it via the CORS proxy. The CORS proxy will simply remove the CORS policy so that you can circumvent the restriction.

For instance, if you're trying to access the API end-point `https://api.example.com/v1`, but it has a CORS policy preventing you from accessing it. To bypass this policy, you can use the CORS that Edge Apps comes with built-in. The way it works is that it prefix your URL with the value from `cors_proxy_url`.

In the example code, you can just use `bypass_cors_url` instead of `api_url` and you can interface with it as per usual.

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
