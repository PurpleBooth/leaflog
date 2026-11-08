# Leaflog documentation

## What it does (today)

Leaflog currently ships its skeleton: the app installs, appears as the
**Leaflog** sidebar tab (admins only), and serves its frontend through
Home Assistant ingress under `/api/hassio_ingress/<token>/` with all
relative URLs, so deep links and refreshes work. The start page reports
the runtime ingress base and the backend health payload.

The grow-diary features (equipment, strains, action and photo log,
forecast, feeding schedules, environment coaching) arrive with the
milestones tracked in the [repository](https://github.com/PurpleBooth/leaflog).

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
- Service calls to Home Assistant are read-only unless you explicitly
  opt in (that switch arrives with the equipment features).

## Troubleshooting

The app must be opened from the Home Assistant sidebar or via ingress;
it refuses direct connections from other addresses (only the Supervisor
ingress proxy may talk to it). Check the add-on log for the listening
port and mode banner.
