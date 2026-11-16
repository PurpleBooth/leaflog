-- Leaflog schema migration V13 (planned veg weeks, spec §4.5).
--
-- The grower's vegetative plan for photoperiod grows feeds the phase
-- forecast: the plan projects the flip until one is logged (D073).
-- Nullable; grows predating this column simply have no plan.

ALTER TABLE grow ADD COLUMN planned_veg_weeks INTEGER;
