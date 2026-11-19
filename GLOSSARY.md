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

**Air**:
The above-ground chart group on the grow page — temperature, humidity, VPD, and CO₂ — under its uppercase micro-label header. The group shares one crosshair-synced connect group and one Chart window with the whole section; it is a UI grouping term, not a sensor type.
_Avoid_: Climate, atmosphere, air zone

**Root zone**:
The below-ground chart group on the grow page — soil moisture, pH, and EC — under its uppercase micro-label header, synced with the Air group's crosshair and window. A UI grouping term for what the substrate and root environment charts show, not a sensor type.
_Avoid_: Substrate, medium, soil zone

**Chart window**:
The rolling time span an environment chart covers, always ending at now; the toggles read 24h / 7d / 30d. A Chart window is never a calendar day, and the word "day" on its own belongs to the Light Cycle.
_Avoid_: Day, Week, Month (as toggle captions), period

**Light Cycle**:
A grow's recurring pattern of grow-light state: one daily lights-on local time plus hours-on per lit phase — a Lights-On Period followed by a Lights-Off Period. Unset parts fall back to sourced preset defaults (photoperiod 18/6 veg and 12/12 flower; autoflowers long days throughout), and reset-to-default restores those defaults. It is not the grow's Photoperiod, and not a chart window. Where a light entity is bound, the entity's observed on/off behaviour overrides the cycle wherever it has data; the cycle remains the editable fallback for gaps and the future. Charts and the phase coach follow the same precedence.
_Avoid_: Light schedule setting, lights config, day/night schedule, lights timer

**Lights-On Period**:
The stretch of a Light Cycle day when the grow lights are on: from the daily lights-on time through lights-on time plus the phase's hours-on. It can run past midnight; the Light Cycle day splits at the lights-on clock time, not at 00:00. Day targets (the day range) apply during it.
_Avoid_: Day (alone), photoperiod (the grow's trait), on time

**Lights-Off Period**:
The rest of a Light Cycle day after the Lights-On Period: the dark hours. Night targets (the night range) apply during it.
_Avoid_: Night (alone), dark period, downtime

**Target Band**:
The recommended range shaded behind an environment chart's readings. It steps between the phase's day range (Lights-On Period targets) and night range (Lights-Off Period targets); phases without grow lights keep one range.
_Avoid_: Safe zone, corridor (in UI text), recommended range, band (alone), banding

### Grow lifecycle

**Medium**:
What the roots grow in: soil, coco, hydro, or a user label. Stored as `substrate`; shown to users as the medium.
_Avoid_: Substrate in UI text, soil as the field name (soil is one value)

**Feeding schedule**:
A reusable feeding chart: products as rows (name, unit, free-text group), dose ranges as stage columns — Propagation, Veg*, Flowering week 1…N, Water. Fitting it to a grow's actual timeline produces the grow's Fitted schedule. Any schedule may be assigned to any grow — the grow's Medium does not restrict which schedules are offered.
_Avoid_: Feed template, nutrient schedule, feed plan, Düngeplan (that is the Fitted schedule)

**Fitted schedule**:
The result of fitting a Feeding schedule to a grow's actual timeline: one row per grow day with the phase, week number and that day's product doses; the plain-water flush tail is protected. Stored per grow and rendered even if the schedule is later deleted; refitting replaces it wholesale.
_Avoid_: Feed plan, Düngeplan (that is the German UI word for this), template (a schedule is never grow-specific)

**Day count**:
Elapsed days since the grow started, shown against the expected total (e.g. 2 / 150).
_Avoid_: Grow age, progress

**Phase day**:
Days elapsed in the current phase, counted from its transition event: day 0 is the transition day. Before any phase event it equals the Day count.
_Avoid_: Stage day, day in flower, grow day (that is the Day count)

**Today marker**:
The line on the harvest forecast chart at the grow's current day count. It must sit over the current phase's window and advance daily.
_Avoid_: Progress line, cursor

**Grow overview**:
The page showing one grow's current state: environment tiles, day count, forecast, charts, and the event timeline. The app lands on the active grow's overview.
_Avoid_: Dashboard, grow progress, home page

**Phase pick**:
Setting the grow's current phase from the interface. A pick records the same transition event a logged action would — picks and actions are one event stream, and the pick re-anchors the forecast windows. A Germination pick moves the grow's start date; other picks never touch it.
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

### Appearance

**Theme source**:
Where the app learns its colours: the parent Home Assistant theme's variables when readable, the built-in theme otherwise. Dark and light follow the same source; there is no separate toggle.
_Avoid_: Colour settings, theme picker, dark-mode toggle
