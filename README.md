# notify-run-rs

This is a rewrite of the [notify.run](https://notify.run) server in Rust for performance. It uses Google Firebase as a backend.

Although the server remains open-source, self-hosting of a notify.run server is no longer a use-case that I prioritize. In particular, non-Firebase databases
will probably never be supported, and documentation is sparse (although all scripts used for production deployment are now part of this repo).
