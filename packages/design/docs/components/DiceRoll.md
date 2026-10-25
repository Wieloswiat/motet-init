# DiceRoll

A dice result: what was rolled, each die, the total and the outcome. In a Scene it is the body of a **SceneEvent** (`caption: false`, since the event's header names the roll), never part of a post.

Faces come from DiceFont (d2, d4, d6, d8, d10, d12, d20, Fate), chosen from `sides` or the expression; dice without a face (d100) show numbers. Each die keeps a text value for assistive technology. Dropped dice are dimmed and struck through, with "dropped" in their name. The outcome icon and colour follow `outcomeTone` (`positive`, `caution`, `danger`) with a sentence that says what happened. A roll is published the moment it is made; editing or deleting the post that follows it never changes it.
