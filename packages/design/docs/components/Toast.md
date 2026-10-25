# Toast

A short platform message about something that just happened.

- `Motet.toast({ message, tone, icon, action: { label, onClick }, duration })`.
- Toasts are a third chrome island: they keep Motet's look inside every World, bottom start, at most three.
- Messages with an action stay until dismissed; others leave after 6 seconds, paused while pointed at or focused. Danger toasts are `role=alert`; the rest are polite status.
- Never put the only copy of important information in a toast.
