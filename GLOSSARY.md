# Leaflog

The shared language of the grow diary. One context for the whole app.

## Language

### Grow environment

**Binding**:
A grow's link from a sensor role to one Home Assistant entity.
_Avoid_: Channel, sensor mapping

**Role**:
The kind of reading a binding provides (temperature, humidity, CO₂, soil moisture, light, reservoir pH, reservoir EC).
_Avoid_: Sensor type, channel kind

**Dead binding**:
A binding whose entity Home Assistant currently reports as unavailable or unknown. Dead bindings are the only ones the grow page may call "no data".
_Avoid_: Broken sensor

**Light Cycle**:
The recurring pattern of grow-light state in a grow: a Lights-On Period followed by a Lights-Off Period. Not the grow's Photoperiod, and not a chart window.
_Avoid_: Day/night schedule, lights timer

**Lights-On Period** / **Lights-Off Period**:
The stretches of a Light Cycle with grow lights on and off. Environment targets may differ between the two.
_Avoid_: Day/night (collides with chart windows and calendar days)

**Target Band**:
The shaded recommended range drawn on an environment chart.
_Avoid_: Safe zone, corridor (in UI text), banding

**Chart window**:
The rolling time range an environment chart covers (24 h, 7 days, 30 days), always ending at now.
_Avoid_: Day/Week/Month as toggle labels ("Day" reads as the Light Cycle's day)

### Grow lifecycle

**Medium**:
What the roots grow in: soil, coco, hydro, or a user label. Stored as `substrate`; shown to users as the medium.
_Avoid_: Substrate in UI text, soil as the field name (soil is one value)

**Day count**:
Elapsed days since the grow started, shown against the expected total (e.g. 2 / 150).
_Avoid_: Grow age, progress

**Today marker**:
The line on the harvest forecast chart at the grow's current day count. It must sit over the current phase's window and advance daily.
_Avoid_: Progress line, cursor

**Grow overview**:
The page showing one grow's current state: environment tiles, day count, forecast, charts, and the event timeline. The app lands on the active grow's overview.
_Avoid_: Dashboard, grow progress, home page

**Phase pick**:
Setting the grow's current phase from the interface. A pick records the same transition event a logged action would — picks and actions are one event stream, and the pick re-anchors the forecast windows.
_Avoid_: Stage, phase override

**Photoperiod**:
A grow whose plant flowers only when light hours drop. Its vegetative time is whatever the grower allows — there is no default length.
_Avoid_: Photo strain, regular

**Autoflower**:
A grow whose plant flowers by age regardless of light; its flip date is estimated, never chosen.
_Avoid_: Auto strain

**Strain label**:
The free-text name of the cultivar a grow was grown from. No catalogue stands behind it: it labels the diary and groups yield history, and it is never a recommendation input.
_Avoid_: Strain database, strain record

### Languages

**Locale source**:
Where the app learns its language: the Home Assistant user language first, the browser locale as fallback, English last.
_Avoid_: Language setting, locale detection

**Locale catalog**:
The per-locale collection of user-facing strings. The frontend's lives in locale files; the backend's is a catalog module for alert and error templates.
_Avoid_: Translation file, strings file

**Key parity**:
The requirement that every locale catalog exposes the same keys as the default (English) catalog, enforced at compile time.
_Avoid_: Translation completeness
