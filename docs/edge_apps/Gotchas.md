## Gotchas

* You need to use **relative** paths to assets (e.g., static/image.svg rather than absolute paths (e.g., /static/image.svg).

---

## Runtime Environment

Security is at the core of Edge Apps, and several measures have been implemented to ensure their security.

### Browser Sandboxing

Edge Apps run inside the browser within appropriate security profiles, ensuring a sandboxed environment.

### Isolated Runtime Environment

The Edge App runtime environment itself is isolated, preventing Edge Apps from communicating or accessing files/data from another Edge App. This isolation extends not only to files and environment data (such as settings) but also to secrets.
