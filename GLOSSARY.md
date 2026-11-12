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
