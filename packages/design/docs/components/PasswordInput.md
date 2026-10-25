# PasswordInput

A password field with a reveal toggle.

- `PasswordInput({ label, help, name, autocomplete })`. `autocomplete` defaults to `current-password`; use `new-password` when creating one.
- The reveal button is a pressed toggle with its own name ("Show password" / "Hide password"). It returns focus to the field.
- While typing with Caps Lock on, a caution line says so. It disappears on blur.
