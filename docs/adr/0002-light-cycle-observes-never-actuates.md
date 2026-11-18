# ADR 0002: The Light Cycle observes reality; Leaflog never actuates grow lights

Status: accepted
Date: 2026-11-17
Decided in: Grow page rework spec (git-bug 95b2e66)
Related: D093/D096 (entity-history precedence for charts and coach), THREAT-MODEL §14 (actuation revisit trigger)

## Context

The Light Cycle needs to know when the grow lights are actually on. There are
two ways to get that knowledge. The app can **observe** — read the bound
light entity's live state and history and override its editable per-phase
defaults wherever the entity has data. Or it can **actuate** — drive a
switch or light on schedule via `call_service`, which AGENTS.md gates behind
an explicit opt-in setting and THREAT-MODEL §14 makes a mandatory
threat-model-revision event. Actuation also collides with the v1 working
agreement of no automatic device control, and would make a diary app a
controller.

## Decision

Leaflog observes, never actuates. The Light Cycle keeps editable per-phase
defaults (reset-to-default intact); a bound light entity's observed on/off
behaviour overrides them wherever it has data — chart bands, coach, and the
edit page's learned caption — and the defaults remain the fallback for gaps
and the future. The app makes no `call_service` on entity switches, now or
behind a setting. The owner explicitly framed this as revisitable: if
Leaflog ever becomes a controller, this ADR is superseded and the AGENTS.md
opt-in gate plus a THREAT-MODEL §14 revision come with that change.

## Alternatives considered

- Actuate the switch per schedule (Leaflog as controller). Rejected: the
  opt-in gate, threat-model revision, and v1 no-device-control agreement
  all push against it, and the grower's HA automation is already the
  source of truth — a second scheduler would fight it.
- Learn observed times into the editable fields. Rejected: writing
  reality back into the defaults invites silent drift between what the
  automation does and what the fields claim; observation stays read-only.

## Consequences

- No `call_service` surface exists for entities; the AGENTS.md gate has
  nothing to gate until this ADR is superseded.
- The Light Cycle editor shows the observed pattern as a caption, derived
  from recent entity history — lazily, with caching, never as a background
  job.
- The editor's per-phase fields remain meaningful for unbound grows and for
  filling history gaps; users who bind an entity get truthful behaviour
  automatically.
- Any future actuation feature starts here: supersede this ADR, add the
  opt-in setting, update THREAT-MODEL §14 in the same change.
