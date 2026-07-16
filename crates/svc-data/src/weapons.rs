//! Weapons index cooking — sc-weapons ship weapons + missiles → serde-clean
//! entries. Reference-catalog framework data (README module rule 4): joins
//! item detail by GUID for the catalog, and feeds mod-langpatch's
//! weapon-enhancer derive.
//!
//! sc-weapons' materialized types aren't serde (same story as the mission
//! registries — see [`crate::missions`]), so only these mirrored views cross
//! into the snapshot. Entries are **flat, one per entity**: pooling by
//! (name_key, desc_key) is cheap plain code, and freezing one consumer's
//! pooling semantics into the snapshot would be premature. Raw locale keys,
//! resolve at query time ([`crate::cooked`] philosophy).
//!
//! FPS weapons are deliberately out of scope for now (no consumer yet; the
//! langpatch parity target only enhanced ship weapons + ordnance).

use sc_holotable::asset::Datacore;
use sc_holotable::asset::generated::EItemSubType;
use sc_holotable::items::Items;
use sc_holotable::weapons::{SustainKind, iter_missiles, iter_ship_weapons};
use serde::{Deserialize, Serialize};

/// The cooked weapons bundle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WeaponsIndex {
    /// Ship-mounted guns, sorted by record name.
    pub ship_weapons: Vec<ShipWeaponEntry>,
    /// Missiles + torpedoes, sorted by record name.
    pub missiles: Vec<MissileEntry>,
}

/// Per-shot damage across all six types.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DamageBreakdown {
    pub physical: f32,
    pub energy: f32,
    pub distortion: f32,
    pub thermal: f32,
    pub biochemical: f32,
    pub stun: f32,
}

impl DamageBreakdown {
    /// Scalar total across all damage types (the "alpha" figure).
    pub fn total(&self) -> f32 {
        self.physical + self.energy + self.distortion + self.thermal + self.biochemical + self.stun
    }
}

/// One ship-mounted gun entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipWeaponEntry {
    /// Record GUID, hex-string form — joins the item catalog.
    pub guid: String,
    /// DCB record name (e.g. `"GATS_BallisticGatling_S1"`).
    pub record_name: String,
    /// Raw `Localization.Name` INI key (`@`-preserved).
    pub name_key: Option<String>,
    /// Raw `Localization.Description` INI key (`@`-preserved).
    pub desc_key: Option<String>,
    /// Weapon size (1–12).
    pub size: i32,
    /// Item subtype DCB string (`"Gun"`, `"Rocket"`, `"NoseMounted"`).
    pub item_sub_type: String,
    /// Per-shot damage. `None` when ammo didn't resolve (mining lasers,
    /// dummies).
    pub damage: Option<DamageBreakdown>,
    /// Ammo penetration distance in metres.
    pub penetration_m: Option<f32>,
    /// Projectile speed in m/s.
    pub ammo_speed: Option<f32>,
    /// Projectile lifetime in seconds (range ≈ speed × lifetime).
    pub ammo_lifetime: Option<f32>,
    /// Physical round budget. `None` for energy weapons.
    pub total_ammo: Option<i32>,
    /// Energy-weapon capacitor budget (`SustainKind::Energy.max_ammo_load`).
    pub capacitor: Option<f32>,
}

/// One missile / torpedo entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissileEntry {
    /// Record GUID, hex-string form — joins the item catalog.
    pub guid: String,
    /// DCB record name (e.g. `"GMISL_S05_IR_TALN_Valkyrie"`).
    pub record_name: String,
    /// Raw `Localization.Name` INI key (`@`-preserved).
    pub name_key: Option<String>,
    /// Raw `Localization.Description` INI key (`@`-preserved).
    pub desc_key: Option<String>,
    /// Missile size class (1–12).
    pub size: i32,
    /// `EItemSubType::Torpedo` vs plain missile.
    pub is_torpedo: bool,
    /// Warhead explosion damage.
    pub damage: Option<DamageBreakdown>,
    /// Cruise speed in m/s. `None` for unguided ordnance.
    pub speed: Option<f32>,
    /// Seconds before the warhead arms after launch.
    pub arm_time: f32,
    /// Guided-missile tracking profile. `None` for unguided ordnance.
    pub tracking: Option<TrackingEntry>,
}

/// Guided-missile tracking profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackingEntry {
    /// Signature-type DCB string the missile homes on (`"Infrared"`,
    /// `"Electromagnetic"`, `"CrossSection"`).
    pub signal: String,
    /// Seconds of tracking required before lock acquires.
    pub lock_time: f32,
    /// Cone half-angle in degrees inside which the missile can lock.
    pub lock_angle_deg: f32,
    /// Minimum lock range in metres.
    pub lock_range_min_m: f32,
    /// Maximum lock range in metres.
    pub lock_range_max_m: f32,
}

/// Cook the weapons index from one parsed `Datacore`. `items` comes from
/// the foundations build the caller already paid for.
pub(crate) fn build_weapons(datacore: &Datacore, items: &Items) -> WeaponsIndex {
    let mut ship_weapons: Vec<ShipWeaponEntry> = iter_ship_weapons(datacore, items)
        .map(|w| ShipWeaponEntry {
            guid: w.guid.to_string(),
            record_name: w.record_name.clone(),
            name_key: w.name_key.as_ref().map(|k| k.as_str().to_string()),
            desc_key: w.desc_key.as_ref().map(|k| k.as_str().to_string()),
            size: w.size,
            item_sub_type: w.item_sub_type.as_dcb_str().to_string(),
            damage: w.damage.as_ref().map(breakdown),
            penetration_m: w.penetration_m,
            ammo_speed: w.ammo_speed,
            ammo_lifetime: w.ammo_lifetime,
            total_ammo: w.total_ammo,
            capacitor: match &w.sustain {
                SustainKind::Energy(e) if e.max_ammo_load > 0.0 => Some(e.max_ammo_load),
                _ => None,
            },
        })
        .collect();
    ship_weapons.sort_by(|a, b| {
        a.record_name
            .cmp(&b.record_name)
            .then_with(|| a.guid.cmp(&b.guid))
    });

    let mut missiles: Vec<MissileEntry> = iter_missiles(datacore, items)
        .map(|m| MissileEntry {
            guid: m.guid.to_string(),
            record_name: m.record_name.clone(),
            name_key: m.name_key.as_ref().map(|k| k.as_str().to_string()),
            desc_key: m.desc_key.as_ref().map(|k| k.as_str().to_string()),
            size: m.size,
            is_torpedo: matches!(m.item_sub_type, EItemSubType::Torpedo),
            damage: m.damage.as_ref().map(breakdown),
            speed: m.speed,
            arm_time: m.arm_time,
            tracking: m.tracking.as_ref().map(|t| TrackingEntry {
                signal: t.signal.as_dcb_str().to_string(),
                lock_time: t.lock_time,
                lock_angle_deg: t.lock_angle_deg,
                lock_range_min_m: t.lock_range_min_m,
                lock_range_max_m: t.lock_range_max_m,
            }),
        })
        .collect();
    missiles.sort_by(|a, b| {
        a.record_name
            .cmp(&b.record_name)
            .then_with(|| a.guid.cmp(&b.guid))
    });

    WeaponsIndex {
        ship_weapons,
        missiles,
    }
}

fn breakdown(d: &sc_holotable::weapons::DamageSummary) -> DamageBreakdown {
    DamageBreakdown {
        physical: d.physical,
        energy: d.energy,
        distortion: d.distortion,
        thermal: d.thermal,
        biochemical: d.biochemical,
        stun: d.stun,
    }
}
