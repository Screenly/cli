## Gotchas

* You need to use **relative** paths to assets (e.g. `static/image.svg` rather than absolute paths (e.g. `/static/image.svg`)

## Runtime Environment

Security is at the core of Edge Apps and there are a number of steps we've taken to ensure they are secure.

The first is the browser sandboxing. Edge Apps are running inside the browser with appropriate security profiles.

Next, the actual Edge App runtime environment is isolated itself. This means that Edge Apps cannot communicate or read files/data from another Edge App. This goes not only for files, but also for environment data (such as settings), but also of course for secrets.
