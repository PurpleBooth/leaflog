# Leaflog documentation

## What it does

Leaflog installs, appears as the **Leaflog** sidebar tab (admins
only), and serves its frontend through Home Assistant ingress under
`/api/hassio_ingress/<token>/` with all relative URLs, so deep links
and refreshes work.

The shipped grow diary covers equipment inventory, strains, an action
and photo log with automatic HA sensor capture, a phase forecast,
feeding schedules fitted to the plant's real timeline, and environment
coaching: VPD, lamp height, substrate-aware pH/EC/TDS guidance and
strain-aware coaching.

## Configuration

| Option | Values | Description |
|---|---|---|
| `log_level` | `debug`, `info`, `warning`, `error` | Verbosity of the backend log (default `info`). |

## Data and backups

Everything Leaflog stores lives in the add-on `/data` directory. Photo
thumbnails are excluded from backups and are regenerated on demand.
Backups keep your database and photos.

## Privacy

- The app never calls the cloud; imports are user-triggered only.
- Uploaded photos are stripped of EXIF/GPS metadata.
- The Supervisor token never reaches the browser and is never logged.
- All Supervisor calls are read-only. The app makes no service calls,
  and no opt-in switch exists yet.

## Troubleshooting

The app must be opened from the Home Assistant sidebar or via ingress;
it refuses direct connections from other addresses (only the Supervisor
ingress proxy may talk to it). Check the add-on log for the listening
port and mode banner.
