# FileUpload

Add files by dropping or choosing them; each file shows its own state.

- `FileUpload({ label, accept, maxBytes, multiple, title, hint })`; emits `files` with `{ id, file, api }`. Report progress with `api.setState(id, 'uploading' | 'ready' | 'error', message, progress)`.
- The drop zone is a `label` for a real file input, so keyboard and screen readers get the platform picker.
- The hint states the formats and size limit. Errors say what to change ("Over the limit of 1 MB"), on the file's own row.
