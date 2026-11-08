"""Leaflog companion integration for HACS.

Leaflog is a Home Assistant app (add-on) — the actual grow diary runs
as a separate container accessible from the sidebar. This minimal
integration exists so HACS can track the repository and deliver
update notifications; it registers no entities and needs no setup.

The app itself is installed through the Home Assistant add-on store:
add https://github.com/PurpleBooth/leaflog as a local repository.
"""

from __future__ import annotations

from homeassistant.core import HomeAssistant
from homeassistant.helpers.typing import ConfigType

DOMAIN = "leaflog"


async def async_setup(hass: HomeAssistant, config: ConfigType) -> bool:
    """Set up the Leaflog companion integration (a no-op marker)."""
    return True
