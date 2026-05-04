// JavaScript injected into the remote entrypoint on every load.
//
// Edge App settings and secrets are available as `screenly_settings`.
// Use this to authenticate the remote page without baking credentials
// into the script.
//
// Example: attach a Bearer token from a setting/secret to every fetch.
//
// const token = screenly_settings.api_token;
// const originalFetch = window.fetch;
// window.fetch = (input, init = {}) => {
//   init.headers = { ...(init.headers || {}), Authorization: `Bearer ${token}` };
//   return originalFetch(input, init);
// };

console.log('screenly_settings:', screenly_settings);
