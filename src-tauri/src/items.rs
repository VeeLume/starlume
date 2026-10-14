//! My Items — the shell side of the module: the inventory fetch (gated),
//! its cached read, and the change event.
//!
//! The network read lives in `svc-dossier`; the gate lives here:
//! [`items_refresh`] passes `require_grpc("items")`, and adds the loadouts
//! only when `require_grpc("loadouts")` passes too. [`items_summary`] reads
//! the on-disk cache and touches no network; [`items_finder`] joins that
//! cache with the cooked game data (`mod-items`).

use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::activity::{Activity, ActivityKind};
use crate::error::AppError;

/// Fired after an inventory fetch. The frontend store reloads on this.
pub const ITEMS_CHANGED_EVENT: &str = "items:changed";

/// What the last inventory fetch holds, in counts — the page header and
/// the first check that the fetch works on a real account.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct InventorySummaryView {
    /// When fetched, epoch seconds; `null` when never fetched.
    pub fetched_at: Option<f64>,
    pub stashes: u32,
    /// Item instances across every stash, attached and stowed ones included.
    pub items: u32,
    /// Stashes whose query failed, with the error.
    pub failed: Vec<FailedStashView>,
    pub entitlements: u32,
    /// Loadout records; `null` when the loadouts feature was off.
    pub loadouts: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct FailedStashView {
    pub inventory_id: String,
    pub error: String,
}

fn summary(inventory: Option<svc_dossier::Inventory>) -> InventorySummaryView {
    let Some(inv) = inventory else {
        return InventorySummaryView {
            fetched_at: None,
            stashes: 0,
            items: 0,
            failed: Vec::new(),
            entitlements: 0,
            loadouts: None,
        };
    };
    fn count(items: &[svc_dossier::inventory::ItemRecord]) -> u32 {
        items
            .iter()
            .map(|i| 1 + count(&i.attached) + count(&i.contents))
            .sum()
    }
    InventorySummaryView {
        fetched_at: Some(inv.fetched_at as f64),
        stashes: inv.stashes.len() as u32,
        items: inv.stashes.iter().map(|s| count(&s.items)).sum(),
        failed: inv
            .failed
            .iter()
            .map(|f| FailedStashView {
                inventory_id: f.location.inventory_id.clone(),
                error: f.error.clone(),
            })
            .collect(),
        entitlements: inv.entitlements.len() as u32,
        loadouts: inv.loadouts.as_ref().map(|l| l.len() as u32),
    }
}

/// The cached inventory, in counts. **No network, no gate.**
#[tauri::command]
#[specta::specta]
pub(crate) fn items_summary(state: tauri::State<'_, AppState>) -> InventorySummaryView {
    summary(state.dossier.cached_inventory())
}

/// Fetch the inventory live from CIG's backend and cache it, keeping the
/// previous fetch and the last fetch per game build (the archive a future
/// "what changed" compares against).
///
/// **Gated** by `require_grpc("items")`; the loadouts ride along only when
/// `require_grpc("loadouts")` passes too. A stale launcher session surfaces
/// as an error the page shows as a sign-in hint.
#[tauri::command]
#[specta::specta]
pub(crate) async fn items_refresh(app: AppHandle) -> Result<InventorySummaryView, AppError> {
    let (dossier, with_loadouts) = {
        let state = app.state::<AppState>();
        state.require_grpc("items")?;
        (
            state.dossier.clone(),
            state.require_grpc("loadouts").is_ok(),
        )
    };
    // The build it's fetched under keys the archive. Rescan (local, cheap)
    // so a patch since the last scan reads as the new build.
    let build = crate::data::refresh_installs(&app)
        .await
        .ok()
        .and_then(|installs| {
            crate::data::default_install(&installs).map(|i| svc_dossier::inventory::BuildTag {
                version: i.version.clone(),
                key: i.build_key(),
            })
        });
    let inventory = {
        let _activity = Activity::start(&app, ActivityKind::Inventory, None, "My items")
            .stage("Fetching from game services…");
        dossier
            .refresh_inventory(crate::dossier::GRPC_USER_AGENT, with_loadouts, build)
            .await?
    };
    let _ = app.emit(ITEMS_CHANGED_EVENT, ());
    Ok(summary(Some(inventory)))
}

// ── The finder ──────────────────────────────────────────────────────────────
//
// specta mirrors of `mod_items` (module crates stay specta-free). Geids cross
// as strings: specta won't export `u64` to TypeScript.

/// Both finder views over the cached inventory.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct FinderView {
    pub items: Vec<OwnedItemView>,
    pub ships: Vec<ShipView>,
    /// Instances the game data couldn't name (a stale build shows here).
    pub unnamed: u32,
    /// When the inventory was fetched, epoch seconds.
    pub fetched_at: f64,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct PlaceView {
    pub name: String,
    /// Parents, nearest first.
    pub parents: Vec<String>,
    /// `storage` | `hangar` | `carried` | `adrift` | `other`.
    pub kind: String,
    /// The location record's GUID (storage and hangar at one place share
    /// it); empty when carried or unknown.
    pub key: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct HolderView {
    pub geid: String,
    pub name: String,
    pub item_type: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ItemKindView {
    pub guid: String,
    pub name: String,
    pub item_type: String,
    pub sub_type: String,
    pub size: i32,
    pub grade: i32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct OwnedItemView {
    pub geid: String,
    pub kind: ItemKindView,
    pub place: PlaceView,
    /// Outermost first (ship, then turret).
    pub holders: Vec<HolderView>,
    pub fitted: bool,
    /// `owned` | `rented` | `bricked` | `unowned`.
    pub ownership: String,
    pub stack: u32,
    pub ammo: Option<u32>,
    pub resource: Option<ResourceStackView>,
    pub crafted: Option<CraftedView>,
    /// What it came with, from its own stock loadout; `null` when unknown
    /// (looted, or loadouts not fetched).
    pub stock: Option<Vec<LoadoutSlotView>>,
    /// The armour set the piece belongs to, from the game data.
    pub set: Option<ArmourSetView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ArmourSetView {
    pub key: String,
    /// "Sol-III Aviator".
    pub name: String,
    /// The pieces the set has in the game data (`Helmet`, `Core`, `Arms`,
    /// `Legs`, `Undersuit`, `Backpack`), in that order.
    pub pieces: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct CraftedView {
    pub ingredients: Vec<IngredientView>,
    pub properties: Vec<CraftedPropertyView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct IngredientView {
    /// `resource` | `item` | `other`.
    pub kind: String,
    pub name: Option<String>,
    pub quality: u16,
    /// µSCU for resources, count for items.
    pub amount: f64,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct CraftedPropertyView {
    /// `CraftingGameplayPropertyDef` CRC.
    pub crc: u32,
    /// The stat's display name; `null` when the game data lacks it.
    pub name: Option<String>,
    /// Raw factor: `0.924` = -7.6%.
    pub factor: f32,
    /// Whether a factor above 1 is the better roll; `null` when unclear.
    pub factor_up_is_better: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ResourceStackView {
    pub name: String,
    pub quality: u16,
    pub scu: f64,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ShipView {
    pub key: String,
    pub kind: ItemKindView,
    pub geid: Option<String>,
    pub place: Option<PlaceView>,
    pub holders: Vec<HolderView>,
    /// `owned` | `rented` | `bricked` | `unowned`.
    pub ownership: String,
    /// `pledge` | `in_game` | `rental` | `unknown`.
    pub source: String,
    /// Epoch seconds.
    pub rental_expiry: Option<f64>,
    pub fitted: Vec<FittedPartView>,
    pub stock: Option<Vec<LoadoutSlotView>>,
    pub saved: Option<Vec<LoadoutSlotView>>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct FittedPartView {
    pub geid: String,
    pub port: Option<String>,
    pub depth: u32,
    pub kind: ItemKindView,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct LoadoutSlotView {
    pub path: Vec<String>,
    /// `null` = an emptied port.
    pub item: Option<ItemKindView>,
}

/// A serde enum's wire name (`"owned"`, `"in_game"`…) — the mirrors carry
/// these as strings rather than duplicating each enum.
fn tag<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn kind_view(k: mod_items::ItemKind) -> ItemKindView {
    ItemKindView {
        guid: k.guid,
        name: k.name,
        item_type: k.item_type,
        sub_type: k.sub_type,
        size: k.size,
        grade: k.grade,
    }
}

fn place_view(p: mod_items::Place) -> PlaceView {
    PlaceView {
        kind: tag(&p.kind),
        name: p.name,
        parents: p.parents,
        key: p.key,
    }
}

fn holder_view(h: mod_items::Holder) -> HolderView {
    HolderView {
        geid: h.geid.to_string(),
        name: h.name,
        item_type: h.item_type,
    }
}

fn slots_view(slots: Option<Vec<mod_items::LoadoutSlot>>) -> Option<Vec<LoadoutSlotView>> {
    slots.map(|s| {
        s.into_iter()
            .map(|slot| LoadoutSlotView {
                path: slot.path,
                item: slot.item.map(kind_view),
            })
            .collect()
    })
}

fn finder_view(f: mod_items::Finder, fetched_at: i64) -> FinderView {
    FinderView {
        items: f
            .items
            .into_iter()
            .map(|i| OwnedItemView {
                geid: i.geid.to_string(),
                ownership: tag(&i.ownership),
                kind: kind_view(i.kind),
                place: place_view(i.place),
                holders: i.holders.into_iter().map(holder_view).collect(),
                fitted: i.fitted,
                stack: i.stack,
                ammo: i.ammo,
                resource: i.resource.map(|r| ResourceStackView {
                    name: r.name,
                    quality: r.quality,
                    scu: r.scu,
                }),
                crafted: i.crafted.map(|c| CraftedView {
                    ingredients: c
                        .ingredients
                        .into_iter()
                        .map(|g| IngredientView {
                            kind: tag(&g.kind),
                            name: g.name,
                            quality: g.quality,
                            amount: g.amount as f64,
                        })
                        .collect(),
                    properties: c
                        .properties
                        .into_iter()
                        .map(|p| CraftedPropertyView {
                            crc: p.crc,
                            name: p.name,
                            factor: p.factor,
                            factor_up_is_better: p.factor_up_is_better,
                        })
                        .collect(),
                }),
                stock: slots_view(i.stock),
                set: i.set.map(|s| ArmourSetView {
                    key: s.key,
                    name: s.name,
                    pieces: s.pieces,
                }),
            })
            .collect(),
        ships: f
            .ships
            .into_iter()
            .map(|s| ShipView {
                ownership: tag(&s.ownership),
                source: tag(&s.source),
                key: s.key,
                kind: kind_view(s.kind),
                geid: s.geid.map(|g| g.to_string()),
                place: s.place.map(place_view),
                holders: s.holders.into_iter().map(holder_view).collect(),
                rental_expiry: s.rental_expiry.map(|t| t as f64),
                fitted: s
                    .fitted
                    .into_iter()
                    .map(|p| FittedPartView {
                        geid: p.geid.to_string(),
                        port: p.port,
                        depth: p.depth,
                        kind: kind_view(p.kind),
                    })
                    .collect(),
                stock: slots_view(s.stock),
                saved: slots_view(s.saved),
            })
            .collect(),
        unnamed: f.unnamed,
        fetched_at: fetched_at as f64,
    }
}

/// The finder over the cached inventory, named from `channel`'s game data.
/// `null` when nothing has been fetched yet. **No network, no gate** — the
/// cache and the cooked snapshot are both local.
#[tauri::command]
#[specta::specta]
pub(crate) async fn items_finder(
    app: AppHandle,
    channel: String,
) -> Result<Option<FinderView>, AppError> {
    let Some(inventory) = app.state::<AppState>().dossier.cached_inventory() else {
        return Ok(None);
    };
    let cooked = crate::data::cooked_for(&app, &channel).await?;
    let fetched_at = inventory.fetched_at;
    let finder = tokio::task::spawn_blocking(move || mod_items::finder(&inventory, &cooked))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Some(finder_view(finder, fetched_at)))
}
