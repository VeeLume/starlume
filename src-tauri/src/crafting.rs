//! Crafting: the shell side of the module. The blueprint catalog is
//! svc-data's reference data, what you marked is `mod-crafting`, and what
//! you hold comes from the cached inventory. Nothing here touches the
//! network, so no online gate applies; owned blueprints arrive through the
//! gated refresh in `dossier.rs`.

use mod_crafting::{CraftingState, Mark};
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::data::{cooked_for, install_for};
use crate::error::AppError;

/// Module id (Rust registry, settings, frontend descriptor).
pub(crate) const MODULE_ID: &str = "crafting";

// ── Views ───────────────────────────────────────────────────────────────────

/// One blueprint in the catalog list.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintRowView {
    pub guid: String,
    pub name: String,
    pub category: Option<String>,
    pub item_type: String,
    pub sub_type: String,
    pub size: i32,
    pub grade: i32,
    pub class: Option<String>,
    pub manufacturer: Option<String>,
    pub craft_seconds: Option<f32>,
    pub materials: Vec<MaterialView>,
}

/// What a recipe's slot takes.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MaterialView {
    pub name: String,
    pub scu: Option<f32>,
    pub count: Option<i32>,
    pub min_quality: i32,
}

/// One blueprint opened: its slots, curves, stat sheet and sources.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintDetailView {
    pub guid: String,
    pub default_quality: i32,
    pub slots: Vec<BlueprintSlotView>,
    pub stats: Vec<BlueprintStatView>,
    pub granted_by: Vec<GrantingMissionView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintSlotView {
    pub name: Option<String>,
    pub material: MaterialView,
    pub modifiers: Vec<SlotModifierView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SlotModifierView {
    pub property: Option<String>,
    pub name: Option<String>,
    /// `percent`, `negatedPercent` or `raw`.
    pub transform: String,
    pub ranges: Vec<ModifierRangeView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModifierRangeView {
    pub additive: bool,
    pub start_quality: i32,
    pub end_quality: i32,
    pub at_start: f32,
    pub at_end: f32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintStatView {
    pub group: Option<String>,
    pub label: String,
    pub property: Option<String>,
    pub unit: String,
    pub higher_is_better: Option<bool>,
    pub base: Option<f32>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GrantingMissionView {
    pub mission_id: String,
    pub title: String,
}

/// What you marked, by blueprint GUID.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CraftingMarksView {
    pub owned: Vec<String>,
    pub want_blueprint: Vec<String>,
    pub want_item: Vec<String>,
}

impl From<CraftingState> for CraftingMarksView {
    fn from(s: CraftingState) -> Self {
        Self {
            owned: s.owned.into_iter().collect(),
            want_blueprint: s.want_blueprint.into_iter().collect(),
            want_item: s.want_item.into_iter().collect(),
        }
    }
}

/// Which mark a toggle flips.
#[derive(Debug, Clone, Copy, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CraftingMark {
    Owned,
    WantBlueprint,
    WantItem,
}

impl From<CraftingMark> for Mark {
    fn from(m: CraftingMark) -> Self {
        match m {
            CraftingMark::Owned => Mark::Owned,
            CraftingMark::WantBlueprint => Mark::WantBlueprint,
            CraftingMark::WantItem => Mark::WantItem,
        }
    }
}

/// One stack of a material you hold.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StockStackView {
    pub name: String,
    pub quality: u32,
    /// SCU for bulk, pieces else.
    pub amount: f64,
    pub bulk: bool,
    pub place: String,
}

// ── Commands ────────────────────────────────────────────────────────────────

/// The catalog for `channel`'s build. Dropped with the rest of the game data
/// on a hide; the next call reads its snapshot again.
async fn index_for(
    app: &AppHandle,
    channel: &str,
) -> Result<std::sync::Arc<svc_data::BlueprintIndex>, AppError> {
    let install = install_for(app, channel).await?;
    app.state::<AppState>()
        .data
        .blueprints(&install)
        .ok_or_else(|| AppError::Config("this build has no cooked blueprints yet".into()))
}

fn material_view(s: &svc_data::BlueprintSlot) -> MaterialView {
    MaterialView {
        name: s.material.clone(),
        scu: s.scu,
        count: s.count,
        min_quality: s.min_quality,
    }
}

/// Every blueprint of `channel`'s build, sorted by name.
#[tauri::command]
#[specta::specta]
pub(crate) async fn crafting_blueprints(
    app: AppHandle,
    channel: String,
) -> Result<Vec<BlueprintRowView>, AppError> {
    let index = index_for(&app, &channel).await?;
    Ok(index
        .blueprints
        .iter()
        .map(|b| BlueprintRowView {
            guid: b.guid.clone(),
            name: b.name.clone(),
            category: b.category.clone(),
            item_type: b.item_type.clone(),
            sub_type: b.sub_type.clone(),
            size: b.size,
            grade: b.grade,
            class: b.class.clone(),
            manufacturer: b.manufacturer.clone(),
            craft_seconds: b.craft_seconds,
            materials: b.slots.iter().map(material_view).collect(),
        })
        .collect())
}

/// One blueprint's recipe; `null` when the build has no such blueprint.
#[tauri::command]
#[specta::specta]
pub(crate) async fn crafting_detail(
    app: AppHandle,
    channel: String,
    blueprint: String,
) -> Result<Option<BlueprintDetailView>, AppError> {
    let index = index_for(&app, &channel).await?;
    let default_quality = index.default_quality;
    Ok(index.get(&blueprint).map(|b| BlueprintDetailView {
        guid: b.guid.clone(),
        default_quality,
        slots: b
            .slots
            .iter()
            .map(|s| BlueprintSlotView {
                name: s.name.clone(),
                material: material_view(s),
                modifiers: s
                    .modifiers
                    .iter()
                    .map(|m| SlotModifierView {
                        property: m.property.clone(),
                        name: m.name.clone(),
                        transform: match m.transform {
                            svc_data::ModifierTransform::FactorToPercent => "percent",
                            svc_data::ModifierTransform::FactorToNegatedPercent => "negatedPercent",
                            svc_data::ModifierTransform::Raw => "raw",
                        }
                        .into(),
                        ranges: m
                            .ranges
                            .iter()
                            .map(|r| ModifierRangeView {
                                additive: r.additive,
                                start_quality: r.start_quality,
                                end_quality: r.end_quality,
                                at_start: r.at_start,
                                at_end: r.at_end,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
        stats: b
            .stats
            .iter()
            .map(|s| BlueprintStatView {
                group: s.group.clone(),
                label: s.label.clone(),
                property: s.property.clone(),
                unit: s.unit.clone(),
                higher_is_better: s.higher_is_better,
                base: s.base,
            })
            .collect(),
        granted_by: b
            .granted_by
            .iter()
            .map(|g| GrantingMissionView {
                mission_id: g.mission_id.clone(),
                title: g.title.clone(),
            })
            .collect(),
    }))
}

/// Serializes state read-modify-writes across commands.
static STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn state_path() -> std::path::PathBuf {
    app_kit::app_data_root().join("crafting").join("state.json")
}

/// What you marked.
#[tauri::command]
#[specta::specta]
pub(crate) fn crafting_marks() -> CraftingMarksView {
    let state: CraftingState = app_kit::load_json(&state_path());
    state.into()
}

/// Flip one mark on a blueprint; the marks as they now stand.
#[tauri::command]
#[specta::specta]
pub(crate) fn crafting_toggle(
    mark: CraftingMark,
    blueprint: String,
) -> Result<CraftingMarksView, AppError> {
    let _lock = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut state: CraftingState = app_kit::load_json(&state_path());
    state.toggle(mark.into(), &blueprint);
    app_kit::save_json(&state_path(), &state)?;
    Ok(state.into())
}

/// The materials in your cached inventory, named from `channel`'s game data;
/// empty when nothing has been fetched yet. **No network, no gate.**
#[tauri::command]
#[specta::specta]
pub(crate) async fn crafting_stock(
    app: AppHandle,
    channel: String,
) -> Result<Vec<StockStackView>, AppError> {
    let Some(inventory) = app.state::<AppState>().dossier.cached_inventory() else {
        return Ok(Vec::new());
    };
    let cooked = cooked_for(&app, &channel).await?;
    let finder = tokio::task::spawn_blocking(move || mod_items::finder(&inventory, &cooked))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(finder
        .items
        .into_iter()
        .filter(|i| i.ownership != mod_items::OwnershipState::Bricked)
        .filter_map(|i| {
            let r = i.resource?;
            // A stack's amount, as My Items counts it: SCU for cargo, pieces
            // for what is gathered by hand.
            let bulk = i.kind.item_type == "Cargo";
            let stack = f64::from(i.stack);
            Some(StockStackView {
                name: r.name,
                quality: u32::from(r.quality),
                amount: if bulk { r.scu * stack } else { stack },
                bulk,
                place: i.place.name,
            })
        })
        .collect())
}
