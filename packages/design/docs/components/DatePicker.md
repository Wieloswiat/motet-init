# DatePicker

A date field you can type into or pick from a calendar, and a time field that names the reader's time zone.

- `DatePicker({ label, value: 'YYYY-MM-DD', min, max, help })`, `Calendar({ value, min, max, isDisabled })`, `TimeField({ label, value: 'HH:MM', step })`.
- The typed format follows the reader's locale; the placeholder spells it out (DD.MM.RRRR in Polish). The full date is written under the field, so 03/04 is never ambiguous.
- The calendar is a grid: arrows move by day and week, Home and End to the week's edges, Page Up and Down by month (with Shift, by year), Enter picks. The week starts where the locale starts it.
- Today has a ring and `aria-current="date"`; the selected day is filled; days outside `min`–`max` are dimmed and `aria-disabled`.
- Time uses the native time input. Deadlines cross time zones, so the field always shows the zone ("UTC+2") with its full name in a tooltip.
