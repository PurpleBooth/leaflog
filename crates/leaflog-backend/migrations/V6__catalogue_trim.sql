-- Leaflog schema migration V6 (catalogue trim, spec §4.1 / D045/D049).
--
-- The equipment catalogue shrinks to durable hardware: light, tent_room,
-- fan (incl. carbon filters) and sensor. Consumables and instruments
-- leave the catalogue — nutrients already live in feed templates,
-- pH/EC/TDS readings in action-log fields, media and pots in notes.
--
-- Conventions as in V1: ULID TEXT ids, RFC 3339 UTC timestamps,
-- snake_case labels of leaflog-core enums validated in Rust.

-- Carbon filters fold into the fan category ("fans (incl. carbon
-- filters)"); the bundled seed file ships them as fan from now on.
UPDATE equipment_item SET category = 'fan' WHERE category = 'carbon_filter';

-- The bundled seed set is trimmed accordingly. Removing a bundled row
-- entirely requires an explicit, documented migration (spec §4.1):
-- these are the sixteen bundled entries whose categories left the
-- catalogue, matched by their exact bundled (name, category). Grow
-- spaces referencing a removed entry lose the reference (ON DELETE
-- CASCADE on grow_space_equipment).
DELETE FROM equipment_item WHERE (name, category) IN (VALUES
    ('AeroStream H05 humidifier',            'humidifier'),
    ('5-in-1 pH/EC meter',                   'meter'),
    ('Fabric pots 20 L (x5)',                'pots_media'),
    ('Royal Mix 50 L substrate',             'pots_media'),
    ('Duct silencer 100 mm',                 'misc'),
    ('Ventilation ducting set 100 mm',       'misc'),
    ('Trim tray set',                        'misc'),
    ('Humidity packs 62% (x8)',              'misc'),
    ('CalMag 1 L',                           'misc'),
    ('Top-Max 500 ml',                       'misc'),
    ('Calmag Up 1 L',                        'misc'),
    ('Mycorrhiza 200 g',                     'misc'),
    ('Yellow sticky traps (x40)',            'misc'),
    ('Air purifier',                         'misc'),
    ('pH calibration buffers (x15)',         'misc'),
    ('Dosing syringes and pipettes',         'misc')
);

-- Any user-added row in a retired category keeps its data but is
-- parked in the legacy `misc` bucket: the API and UI no longer offer
-- retired categories, and a stored misc row cannot be re-created or
-- updated as misc — an update payload must pick a catalogue category.
UPDATE equipment_item
SET category = 'misc'
WHERE category NOT IN ('light', 'tent_room', 'fan', 'sensor', 'misc');
