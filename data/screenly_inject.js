// JavaScript injected into the remote entrypoint on every load.
//
// Edge App settings and secrets are available as `screenly_settings`.
// Use this to authenticate the remote page without baking credentials
// into the script.
//
// This script runs AFTER the page has fully loaded — DOMContentLoaded /
// window.load have already fired, so manipulate the DOM directly. Don't
// wrap your code in `document.addEventListener('DOMContentLoaded', ...)`
// (the listener will be registered too late and never fire).
//
// ---- Helpers --------------------------------------------------------------

// Set an input's value and notify listeners.
function setValue(selector, value) {
  const el = document.querySelector(selector);
  if (!el) return false;
  el.value = value;
  el.dispatchEvent(new Event('change', { bubbles: true }));
  return true;
}

// Set an input's value through the native setter so frameworks like React
// pick it up. Use this when `setValue` doesn't take effect.
function setReactValue(selector, value) {
  const el = document.querySelector(selector);
  if (!el) return false;
  const setter = Object.getOwnPropertyDescriptor(
    HTMLInputElement.prototype,
    'value'
  ).set;
  setter.call(el, value);
  el.dispatchEvent(new Event('input', { bubbles: true }));
  return true;
}

// Set a cookie scoped to `domain` and reload the page. No-ops if already set.
function setCookie(key, value, domain) {
  if (document.cookie.split('; ').some(c => c.startsWith(key + '='))) return;
  document.cookie = `${key}=${value}; path=/; domain=${domain}`;
  location.reload();
}

// Run `fn` only when the current path matches `path`.
function onPath(path, fn) {
  if (location.pathname === path) fn();
}

// ---- Examples (uncomment one) --------------------------------------------

// Form-fill login on /login:
//
// onPath('/login', () => {
//   if (setValue('input[name="username"]', screenly_settings.username) &&
//       setValue('input[name="password"]', screenly_settings.password)) {
//     document.querySelector('button[type="submit"]').click();
//   } else {
//     setTimeout(arguments.callee, 1000); // retry until the form is rendered
//   }
// });

// SSO via cookie:
//
// setCookie('session_id', screenly_settings.session_id, '.example.com');

// Override fetch to attach a Bearer token to every request the page makes:
//
// const token = screenly_settings.api_token;
// const originalFetch = window.fetch;
// window.fetch = (input, init = {}) => {
//   init.headers = { ...(init.headers || {}), Authorization: `Bearer ${token}` };
//   return originalFetch(input, init);
// };

// ---- Default: log what's available ---------------------------------------

console.log('screenly_settings:', screenly_settings);
