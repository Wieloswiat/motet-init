# Shape

Corner geometry, frame profiles and the radius scale.

Worlds pick a corner (`round`, `squircle`, `bevel`, `square`), a scale, and a frame profile for large framed surfaces: `even`, `cut` (one chamfered corner) or `clipped` (two opposite chamfers). Use the profile for hard-edged Worlds instead of corner ornaments. Controls take the corner but never the profile. Without `corner-shape` support, bevels fall back to smaller round radii and profiles to small even corners. Framed surfaces read `--frame-radius`, `--frame-shape` and `--frame-line`.
