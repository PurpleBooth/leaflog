# Leaflog documentation

## What it does

Leaflog installs, appears as the **Leaflog** sidebar tab (admins
only), and serves its frontend through Home Assistant ingress under
`/api/hassio_ingress/<token>/` with all relative URLs, so deep links
and refreshes work.

The shipped grow diary covers grows and plants. Each grow carries its
strain label, photoperiod and Advanced Settings. Around them sit an
action and photo log with automatic HA sensor capture through
per-grow bindings, a phase forecast, and feeding schedules fitted to
the plant's real timeline. Environment coaching covers VPD, lamp
height and substrate-aware pH/EC/TDS guidance.

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

## Verifying the image

Every published image is signed with [cosign](https://docs.sigstore.dev/cosign/signing/overview/) (keyless, Sigstore), scanned with Trivy, and carries an SBOM. The Supervisor does not check signatures, so this is optional; to verify one yourself:

```sh
cosign verify \
  --certificate-identity 'https://github.com/PurpleBooth/leaflog/.github/workflows/image.yml@refs/tags/v0.22.0' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  ghcr.io/purplebooth/leaflog@sha256:<digest>
```

Use the digest from `docker buildx imagetools inspect ghcr.io/purplebooth/leaflog:<version>`.

## Troubleshooting

The app must be opened from the Home Assistant sidebar or via ingress;
it refuses direct connections from other addresses (only the Supervisor
ingress proxy may talk to it). Check the add-on log for the listening
port and mode banner.
